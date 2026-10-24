use super::{
    dto::{Direction, Filters, MemoryProgress},
    history, personal,
};
use crate::{
    entities::{
        flashcard_attempts as attempts, flashcard_memory as memory, vocabulary_cards as cards,
    },
    AppError,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};
use std::collections::BTreeMap;

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
    let mut result = MemoryProgress {
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
        format!("SELECT c.word_key, m.* FROM ({}) c JOIN flashcard_memory m ON m.user_id = ${user_param} AND m.identity = jsonb_build_array(c.word_key, c.locale, c.part_of_speech) WHERE m.reviews > 0 AND m.direction IN ({directions})", source.sql), values)).await?;
    let memories = rows
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String>("", "word_key")?,
                memory::Model::from_query_result(&row, "")?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let profile = personal::load(db, user).await?;
    let parameters = fsrs::check_and_fill_parameters(&profile.parameters)
        .map_err(|_| AppError::Internal("Tham số FSRS không hợp lệ.".into()))?;
    // FSRS::new clips w[20] to this range internally; the public recall
    // helper accepts decay directly rather than a scheduler instance.
    let decay = parameters[20].clamp(0.1, 0.8);
    // A personalized model may have advanced since these cards were reviewed.
    // Fetch all stale histories together, avoiding a query per vocabulary word.
    let stale_keys = memories
        .iter()
        .filter(|(_, m)| m.parameter_version != profile.version())
        .map(|(_, m)| m.study_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    type ChainKey = (i64, String, String);
    let mut histories = BTreeMap::<ChainKey, Vec<attempts::Model>>::new();
    if !stale_keys.is_empty() {
        let attempts = attempts::Entity::find()
            .filter(attempts::Column::UserId.eq(user))
            .filter(attempts::Column::StudyKey.is_in(stale_keys))
            .filter(attempts::Column::Direction.is_in(direction.directions().iter().copied()))
            .filter(attempts::Column::Rating.is_not_null())
            .filter(attempts::Column::ReviewedAt.is_not_null())
            .order_by_asc(attempts::Column::ReviewedAt)
            .order_by_asc(attempts::Column::CreatedAt)
            .order_by_asc(attempts::Column::Id)
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
    let scheduler = profile.scheduler()?;
    let mut words = BTreeMap::<String, f64>::new();
    for (word, mut state) in memories {
        if state.parameter_version != profile.version() {
            let rows = histories
                .remove(&(
                    state.card_id,
                    state.direction.clone(),
                    state.study_key.clone(),
                ))
                .unwrap_or_default();
            let item = history::current_item(rows, state.reviews)?;
            let replayed = scheduler
                .memory_state(item, None)
                .map_err(|_| AppError::Internal("Không khôi phục được trí nhớ FSRS.".into()))?;
            state.stability = replayed.stability;
            state.difficulty = replayed.difficulty;
        }
        let days = (now - state.reviewed_at).num_milliseconds().max(0) as f32 / 86_400_000.0;
        let recall = fsrs::current_retrievability(
            fsrs::MemoryState {
                stability: state.stability,
                difficulty: state.difficulty,
            },
            days,
            decay,
        ) as f64;
        if !recall.is_finite() {
            return Err(AppError::Internal(
                "Không tính được tiến độ ghi nhớ.".into(),
            ));
        }
        let recall = recall.clamp(0.0, 1.0);
        words
            .entry(word)
            .and_modify(|p| *p = p.min(recall))
            .or_insert(recall);
    }
    let learned = (words.len() as u64).min(total);
    result.remembered = words.values().sum::<f64>().min(learned as f64);
    result.forgotten = learned as f64 - result.remembered;
    result.unseen = total - learned;
    Ok(result)
}
