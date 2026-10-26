use super::{
    dto::{Direction, Filters, MemoryProgress},
    history, personal,
};
use crate::{
    entities::{flashcard_attempts as attempts, vocabulary_cards as cards},
    AppError,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use std::collections::BTreeMap;

#[derive(FromQueryResult)]
struct RecallMemory {
    word_key: String,
    card_id: i64,
    direction: String,
    study_key: String,
    parameter_version: i32,
    stability: f32,
    difficulty: f32,
    reviews: i32,
    reviewed_at: chrono::DateTime<chrono::FixedOffset>,
}
type ChainKey = (i64, String, String);
type Histories = BTreeMap<ChainKey, Vec<history::ReviewRow>>;

/// Expected word counts, not a claim that individual words are known/forgotten.
/// Sources and meanings share memory; a word with multiple studied schedules
/// uses the weakest recall probability in the configured review directions.
pub async fn estimate<C: ConnectionTrait>(
    db: &C,
    user: i64,
    filters: &Filters,
    direction: Direction,
    total: u64,
) -> Result<MemoryProgress, AppError> {
    let now = chrono::Utc::now().fixed_offset();
    let result = MemoryProgress {
        remembered: 0.0,
        forgotten: 0.0,
        unseen: total,
        total,
        as_of: now,
    };
    if total == 0 {
        return Ok(result);
    }
    let source = super::super::library::filtered(
        super::super::library::within_course(
            super::super::library::scope(user, filters.session_id),
            filters.course_id,
        ),
        filters.topic.clone(),
        filters.q.clone(),
    )
    .filter(cards::Column::Meaning.ne(""))
    .select_only()
    .columns([
        cards::Column::WordKey,
        cards::Column::Locale,
        cards::Column::PartOfSpeech,
    ])
    .distinct()
    .build(sea_orm::DbBackend::Postgres);
    let mut values = source.values.map(|v| v.0).unwrap_or_default();
    let user_param = values.len() + 1;
    values.push(user.into());
    // These strings come exclusively from the Direction enum, never user SQL.
    let directions = direction
        .directions()
        .iter()
        .map(|d| format!("'{d}'"))
        .collect::<Vec<_>>()
        .join(",");
    let rows = db.query_all(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres,
        format!("SELECT c.word_key, m.card_id, m.direction, m.study_key, m.parameter_version, m.stability, m.difficulty, m.reviews, m.reviewed_at FROM ({}) c JOIN flashcard_memory m ON m.user_id = ${user_param} AND m.identity = jsonb_build_array(c.word_key, c.locale, c.part_of_speech) WHERE m.reviews > 0 AND m.direction IN ({directions})", source.sql), values)).await?;
    let memories = rows
        .into_iter()
        .map(|row| RecallMemory::from_query_result(&row, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let profile = personal::load(db, user).await?;
    // A personalized model may have advanced since these cards were reviewed.
    // Fetch all stale histories together, avoiding a query per vocabulary word.
    let stale_keys = memories
        .iter()
        .filter(|m| m.parameter_version != profile.version())
        .map(|m| m.study_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let replay_count = stale_keys.len();
    let mut histories = Histories::new();
    if !stale_keys.is_empty() {
        let attempts = history::select()
            .filter(attempts::Column::UserId.eq(user))
            .filter(attempts::Column::StudyKey.is_in(stale_keys))
            .filter(attempts::Column::Direction.is_in(direction.directions().iter().copied()))
            .filter(attempts::Column::Rating.is_not_null())
            .filter(attempts::Column::ReviewedAt.is_not_null())
            .order_by_asc(attempts::Column::ReviewedAt)
            .order_by_asc(attempts::Column::CreatedAt)
            .order_by_asc(attempts::Column::Id)
            .into_model::<history::ReviewRow>()
            .all(db)
            .await?;
        for attempt in attempts {
            histories
                .entry((
                    attempt.memory_card_id,
                    attempt.direction.clone(),
                    attempt.study_key.clone(),
                ))
                .or_default()
                .push(attempt);
        }
    }
    // Keep this GET read-only: persisting replay could race a grade and overwrite its
    // newer memory. Large replays run on Tokio's blocking pool, not request workers.
    let heavy = replay_count >= 32 || histories.values().map(Vec::len).sum::<usize>() > 512;
    let compute = move || calculate(result, memories, histories, profile);
    if heavy {
        tokio::task::spawn_blocking(compute)
            .await
            .map_err(|_| AppError::Internal("Không tính được tiến độ ghi nhớ.".into()))?
    } else {
        compute()
    }
}

fn calculate(
    mut result: MemoryProgress,
    memories: Vec<RecallMemory>,
    mut histories: Histories,
    profile: personal::Profile,
) -> Result<MemoryProgress, AppError> {
    let parameters = fsrs::check_and_fill_parameters(&profile.parameters)
        .map_err(|_| AppError::Internal("Tham số FSRS không hợp lệ.".into()))?;
    let decay = parameters[20].clamp(0.1, 0.8);
    let scheduler = profile.scheduler()?;
    let mut words = BTreeMap::<String, f64>::new();
    for mut state in memories {
        if state.parameter_version != profile.version() {
            let rows = histories
                .remove(&(
                    state.card_id,
                    state.direction.clone(),
                    state.study_key.clone(),
                ))
                .unwrap_or_default();
            match history::current_item(rows, state.reviews).and_then(|item| {
                scheduler
                    .memory_state(item, None)
                    .map_err(|_| AppError::Internal("Không khôi phục được trí nhớ FSRS.".into()))
            }) {
                Ok(replayed) => {
                    state.stability = replayed.stability;
                    state.difficulty = replayed.difficulty;
                }
                Err(error) => tracing::warn!(card_id = state.card_id, %error,
                    "Using saved FSRS memory because replay history is invalid"),
            }
        }
        if !state.stability.is_finite() || state.stability <= 0.0 || !state.difficulty.is_finite() {
            tracing::warn!(
                card_id = state.card_id,
                "Skipping invalid FSRS memory estimate"
            );
            continue;
        }
        let days =
            (result.as_of - state.reviewed_at).num_milliseconds().max(0) as f32 / 86_400_000.0;
        let recall = fsrs::current_retrievability(
            fsrs::MemoryState {
                stability: state.stability,
                difficulty: state.difficulty,
            },
            days,
            decay,
        ) as f64;
        if !recall.is_finite() {
            continue;
        }
        let recall = recall.clamp(0.0, 1.0);
        words
            .entry(state.word_key)
            .and_modify(|p| *p = p.min(recall))
            .or_insert(recall);
    }
    let learned = (words.len() as u64).min(result.total);
    result.remembered = words.values().sum::<f64>().min(learned as f64);
    result.forgotten = learned as f64 - result.remembered;
    result.unseen = result.total - learned;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_histories_use_saved_memory_and_one_invalid_card_does_not_fail_large_overview() {
        let now = chrono::Utc::now().fixed_offset();
        let memories = (0..64)
            .map(|id| RecallMemory {
                word_key: format!("word{id}"),
                card_id: id,
                direction: "vi_en".into(),
                study_key: format!("key{id}"),
                parameter_version: -1,
                stability: if id == 0 { f32::NAN } else { 10.0 },
                difficulty: 5.0,
                reviews: 1,
                reviewed_at: now - chrono::Duration::days(10),
            })
            .collect();
        let result = calculate(
            MemoryProgress {
                remembered: 0.0,
                forgotten: 0.0,
                unseen: 64,
                total: 64,
                as_of: now,
            },
            memories,
            Histories::new(),
            personal::Profile {
                parameter_version: 0,
                model: None,
                parameters: fsrs::DEFAULT_PARAMETERS.to_vec(),
            },
        )
        .unwrap();
        assert_eq!(result.unseen, 1);
        assert!((result.remembered - 63.0 * 0.9).abs() < 0.001);
        assert!((result.forgotten - 63.0 * 0.1).abs() < 0.001);
    }
}
