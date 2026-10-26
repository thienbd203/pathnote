use super::dto::{Configuration, Direction, Filters, Overview};
use crate::{
    entities::{
        flashcard_memory as memory, flashcard_settings as settings, places, sessions,
        vocabulary_cards as cards,
    },
    modules::notes::service as notes,
    AppError,
};
use sea_orm::{
    sea_query::{Alias, Expr, JoinType},
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, RelationTrait,
};
use serde_json::Value;
pub fn key(card: &cards::Model) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identity(card)).expect("String serialization"))
    )
}
pub fn identity(card: &cards::Model) -> Value {
    serde_json::to_value((&card.word_key, &card.locale, &card.part_of_speech))
        .expect("String tuple serialization")
}
// A shared schedule is identified by the word and its part of speech. Tickets
// separately retain the source content hash so edits still invalidate them.
pub fn content_key(card: &cards::Model) -> String {
    use sha2::{Digest, Sha256};
    let content = (
        &card.word,
        &card.meaning,
        &card.locale,
        &card.part_of_speech,
    );
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&content).expect("String tuple serialization"))
    )
}
pub async fn meanings<C: ConnectionTrait>(
    db: &C,
    user: i64,
    card: &cards::Model,
) -> Result<String, AppError> {
    Ok(meanings_for(db, user, &[card])
        .await?
        .remove(&card.id)
        .unwrap_or_else(|| card.meaning.clone()))
}
/// Batch legacy history fallbacks instead of issuing one meanings query per log row.
pub(super) async fn meanings_for<C: ConnectionTrait>(
    db: &C,
    user: i64,
    items: &[&cards::Model],
) -> Result<std::collections::HashMap<i64, String>, AppError> {
    if items.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut identities = Condition::any();
    let mut seen = std::collections::BTreeSet::new();
    for card in items {
        if seen.insert((&card.word_key, &card.locale, &card.part_of_speech)) {
            identities = identities.add(
                Condition::all()
                    .add(cards::Column::WordKey.eq(&card.word_key))
                    .add(cards::Column::Locale.eq(&card.locale))
                    .add(cards::Column::PartOfSpeech.eq(&card.part_of_speech)),
            );
        }
    }
    let rows = super::super::library::scope(user, None)
        .filter(identities)
        .filter(cards::Column::Meaning.ne(""))
        .order_by_asc(cards::Column::Id)
        .select_only()
        .columns([
            cards::Column::WordKey,
            cards::Column::Locale,
            cards::Column::PartOfSpeech,
            cards::Column::Meaning,
        ])
        .into_tuple::<(String, String, String, String)>()
        .all(db)
        .await?;
    let mut grouped =
        std::collections::BTreeMap::<_, std::collections::BTreeMap<String, String>>::new();
    for (word, locale, pos, meaning) in rows {
        grouped
            .entry((word, locale, pos))
            .or_default()
            .entry(super::super::sync::normalize(&meaning))
            .or_insert(meaning);
    }
    Ok(items
        .iter()
        .map(|card| {
            let meaning = grouped
                .get(&(
                    card.word_key.clone(),
                    card.locale.clone(),
                    card.part_of_speech.clone(),
                ))
                .map(|meanings| meanings.values().cloned().collect::<Vec<_>>().join("; "))
                // History may reference an inactive card after its block was removed.
                // There is no active cluster then, but its unchanged source meaning is still known.
                .unwrap_or_else(|| card.meaning.clone());
            (card.id, meaning)
        })
        .collect())
}
pub async fn authorize<C: ConnectionTrait>(
    db: &C,
    user: i64,
    filters: &Filters,
) -> Result<(), AppError> {
    if let Some(id) = filters.course_id {
        crate::modules::courses::service::model(db, user, id).await?;
    }
    if let Some(id) = filters.session_id {
        notes::access(db, user, id, false).await?;
    }
    Ok(())
}
pub async fn configuration<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
) -> Result<(Direction, f32), AppError> {
    let config = settings::Entity::find_by_id((user, f.scope()))
        .one(db)
        .await?;
    Ok(config.map_or((Direction::ViEn, 0.9), |c| {
        (
            match c.direction.as_str() {
                "en_vi" => Direction::EnVi,
                "both" => Direction::Both,
                _ => Direction::ViEn,
            },
            c.retention,
        )
    }))
}
pub async fn memory<C: ConnectionTrait>(
    db: &C,
    user: i64,
    card: &cards::Model,
    direction: &str,
) -> Result<Option<memory::Model>, AppError> {
    Ok(
        memory::Entity::find_by_id((user, key(card), direction.to_owned()))
            .one(db)
            .await?
            .filter(|m| m.study_key == key(card)),
    )
}
fn base(user: i64, f: &Filters, direction: &str) -> sea_orm::Select<cards::Entity> {
    let mut query = super::super::library::filtered(
        super::super::library::within_course(
            super::super::library::scope(user, f.session_id),
            f.course_id,
        ),
        f.topic.clone(),
        f.q.clone(),
    )
    .filter(cards::Column::Meaning.ne(""));
    QuerySelect::query(&mut query).join(JoinType::LeftJoin, Alias::new("flashcard_memory"), Condition::all()
        .add(Expr::col((Alias::new("flashcard_memory"), memory::Column::UserId)).eq(user))
        .add(Expr::col((Alias::new("flashcard_memory"), memory::Column::Direction)).eq(direction))
        .add(Expr::cust("flashcard_memory.identity = jsonb_build_array(vocabulary_cards.word_key, vocabulary_cards.locale, vocabulary_cards.part_of_speech)")));
    query
}
fn new_condition() -> sea_orm::sea_query::SimpleExpr {
    Expr::col((Alias::new("flashcard_memory"), memory::Column::CardId)).is_null()
}
fn due_condition(now: chrono::DateTime<chrono::FixedOffset>) -> sea_orm::sea_query::SimpleExpr {
    Expr::col((Alias::new("flashcard_memory"), memory::Column::DueAt)).lte(now)
}
pub async fn overview<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
) -> Result<Overview, AppError> {
    overview_with_settings(db, user, f, configuration(db, user, f).await?).await
}
pub async fn overview_with_settings<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
    (direction, retention): (Direction, f32),
) -> Result<Overview, AppError> {
    let mut overview = counts(db, user, f, (direction, retention)).await?;
    let average = db.query_one(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres,
        "SELECT AVG(seconds)::double precision AS average FROM (SELECT EXTRACT(EPOCH FROM (reviewed_at-created_at)) AS seconds FROM flashcard_attempts WHERE user_id=$1 AND reviewed_at IS NOT NULL ORDER BY reviewed_at DESC LIMIT 100) recent WHERE seconds BETWEEN 1 AND 120 HAVING COUNT(*) >= 10",[user.into()])).await?.and_then(|row|row.try_get::<Option<f64>>("","average").ok().flatten());
    overview.memory_progress =
        Some(super::progress::estimate(db, user, f, direction, overview.word_count).await?);
    overview.estimated_seconds = average.map(|seconds| {
        (seconds * (overview.new + if f.new_only { 0 } else { overview.due }) as f64).round() as u64
    });
    Ok(overview)
}
/// One bounded aggregate for every configured direction; next does not read estimate history.
pub async fn counts<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
    (direction, retention): (Direction, f32),
) -> Result<Overview, AppError> {
    let source = super::super::library::filtered(
        super::super::library::within_course(
            super::super::library::scope(user, f.session_id),
            f.course_id,
        ),
        f.topic.clone(),
        f.q.clone(),
    )
    .select_only()
    .columns([
        cards::Column::WordKey,
        cards::Column::Meaning,
        cards::Column::Locale,
        cards::Column::PartOfSpeech,
    ])
    .build(sea_orm::DbBackend::Postgres);
    let mut values = source.values.map(|values| values.0).unwrap_or_default();
    let user_param = values.len() + 1;
    values.push(user.into());
    let now_param = values.len() + 1;
    values.push(chrono::Utc::now().fixed_offset().into());
    // Direction is an internal enum. All user-supplied filters remain bound parameters.
    let directions = direction
        .directions()
        .iter()
        .map(|d| format!("('{d}')"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"SELECT d.direction,
        COUNT(*) FILTER (WHERE c.reviewable AND m.card_id IS NULL) AS new_count,
        COUNT(*) FILTER (WHERE c.reviewable AND m.due_at <= ${now_param}) AS due_count,
        COUNT(*) FILTER (WHERE c.reviewable AND m.step > 0) AS learning_count,
        MIN(m.due_at) FILTER (WHERE c.reviewable AND m.due_at > ${now_param}) AS next_due,
        COUNT(*) FILTER (WHERE c.reviewable) AS reviewable,
        COUNT(DISTINCT c.word_key) AS word_count
        FROM (SELECT word_key, locale, part_of_speech, BOOL_OR(meaning <> '') AS reviewable FROM ({source_sql}) source GROUP BY word_key, locale, part_of_speech) c CROSS JOIN (VALUES {directions}) d(direction)
        LEFT JOIN flashcard_memory m ON m.user_id = ${user_param} AND m.direction = d.direction
          AND m.identity = jsonb_build_array(c.word_key, c.locale, c.part_of_speech)
        GROUP BY d.direction"#,
        source_sql = source.sql
    );
    let rows = db
        .query_all(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            sql,
            values,
        ))
        .await?;
    let mut result = Overview {
        estimated_seconds: None,
        settings: Configuration {
            direction,
            retention,
        },
        new: 0,
        due: 0,
        learning: 0,
        next_due_at: None,
        algorithm: "FSRS-6",
        reviewable: 0,
        word_count: 0,
        memory_progress: None,
    };
    for row in rows {
        result.new += row.try_get::<i64>("", "new_count")? as u64;
        result.due += row.try_get::<i64>("", "due_count")? as u64;
        result.learning += row.try_get::<i64>("", "learning_count")? as u64;
        result.reviewable = row.try_get::<i64>("", "reviewable")? as u64;
        result.word_count = row.try_get::<i64>("", "word_count")? as u64;
        if let Some(next) =
            row.try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", "next_due")?
        {
            result.next_due_at = Some(result.next_due_at.map_or(next, |old| old.min(next)));
        }
    }
    Ok(result)
}
pub(super) struct Queued {
    pub card: cards::Model,
    pub direction: &'static str,
    pub memory: Option<memory::Model>,
    pub source: super::dto::Source,
}
#[derive(FromQueryResult)]
struct Candidate {
    #[sea_orm(nested)]
    card: cards::Model,
    source_title: String,
    source_owner: i64,
}

pub async fn next<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
    direction: Direction,
) -> Result<Option<Queued>, AppError> {
    let now = chrono::Utc::now().fixed_offset();
    let mut candidates = Vec::new();
    for d in direction.directions() {
        let eligible = if f.new_only {
            Condition::any().add(new_condition())
        } else {
            Condition::any()
                .add(new_condition())
                .add(due_condition(now))
        };
        let mut query = base(user, f, d).filter(eligible);
        QuerySelect::query(&mut query)
            .order_by_expr(
                Expr::cust("flashcard_memory.due_at IS NULL"),
                sea_orm::Order::Asc,
            )
            .order_by_expr(
                Expr::col((Alias::new("flashcard_memory"), memory::Column::DueAt)).into(),
                sea_orm::Order::Asc,
            )
            .order_by_expr(Expr::cust("random()"), sea_orm::Order::Asc);
        let selected = query
            .join(JoinType::InnerJoin, cards::Relation::Session.def())
            .join(JoinType::InnerJoin, sessions::Relation::Place.def())
            .select_also(memory::Entity)
            .expr_as(
                Expr::col((sessions::Entity, sessions::Column::Title)),
                "A_source_title",
            )
            .expr_as(
                Expr::col((places::Entity, places::Column::UserId)),
                "A_source_owner",
            )
            .limit(1)
            .into_model::<Candidate, memory::Model>()
            .one(db)
            .await?;
        if let Some((candidate, m)) = selected {
            let card = candidate.card;
            let m = m.filter(|m| m.study_key == key(&card));
            let source = super::dto::Source {
                id: card.session_id,
                title: candidate.source_title,
                shared: candidate.source_owner != user,
            };
            candidates.push((
                m.as_ref().map(|m| m.due_at),
                Queued {
                    card,
                    direction: d,
                    memory: m,
                    source,
                },
            ));
        }
    }
    use rand::seq::SliceRandom;
    candidates.shuffle(&mut rand::thread_rng());
    candidates.sort_by_key(|(due, _)| (due.is_none(), *due));
    Ok(candidates.into_iter().next().map(|(_, queued)| queued))
}
