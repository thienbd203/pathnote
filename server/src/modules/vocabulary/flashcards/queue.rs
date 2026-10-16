use super::dto::{Configuration, Direction, Filters, Overview};
use crate::{
    entities::{
        flashcard_memory as memory, flashcard_settings as settings, vocabulary_cards as cards,
    },
    modules::notes::service as notes,
    AppError,
};
use sea_orm::{
    sea_query::{Alias, Expr, JoinType},
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
};
use serde_json::{json, Value};
pub fn key(card: &cards::Model) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identity(card)).expect("String serialization"))
    )
}
pub fn identity(card: &cards::Model) -> Value {
    json!([card.word, card.meaning, card.locale, card.part_of_speech])
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
        memory::Entity::find_by_id((user, card.id, direction.to_owned()))
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
        .add(Expr::col((Alias::new("flashcard_memory"), memory::Column::CardId)).equals((cards::Entity,cards::Column::Id)))
        .add(Expr::cust("flashcard_memory.identity = jsonb_build_array(vocabulary_cards.word, vocabulary_cards.meaning, vocabulary_cards.locale, vocabulary_cards.part_of_speech)")));
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
    let now = chrono::Utc::now().fixed_offset();
    let (mut new, mut due, mut learning) = (0, 0, 0);
    let mut next_due: Option<chrono::DateTime<chrono::FixedOffset>> = None;
    for d in direction.directions() {
        let query = base(user, f, d);
        new += query.clone().filter(new_condition()).count(db).await?;
        due += query.clone().filter(due_condition(now)).count(db).await?;
        learning += query
            .clone()
            .filter(Expr::col((Alias::new("flashcard_memory"), memory::Column::Step)).gt(0))
            .count(db)
            .await?;
        let next: Option<chrono::DateTime<chrono::FixedOffset>> = query
            .filter(Expr::col((Alias::new("flashcard_memory"), memory::Column::DueAt)).gt(now))
            .select_only()
            .expr(Expr::col((Alias::new("flashcard_memory"), memory::Column::DueAt)).min())
            .into_tuple::<Option<chrono::DateTime<chrono::FixedOffset>>>()
            .one(db)
            .await?
            .flatten();
        if let Some(next) = next.filter(|n| *n > now) {
            next_due = Some(next_due.map_or(next, |old| old.min(next)));
        }
    }
    let average = db.query_one(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres,
        "SELECT AVG(seconds)::double precision AS average FROM (SELECT EXTRACT(EPOCH FROM (reviewed_at-created_at)) AS seconds FROM flashcard_attempts WHERE user_id=$1 AND reviewed_at IS NOT NULL ORDER BY reviewed_at DESC LIMIT 100) recent WHERE seconds BETWEEN 1 AND 120 HAVING COUNT(*) >= 10",[user.into()])).await?.and_then(|row|row.try_get::<Option<f64>>("","average").ok().flatten());
    let estimate = average
        .map(|seconds| (seconds * (new + if f.new_only { 0 } else { due }) as f64).round() as u64);
    Ok(Overview {
        estimated_seconds: estimate,
        settings: Configuration {
            direction,
            retention,
        },
        new,
        due,
        learning,
        next_due_at: next_due,
        algorithm: "FSRS-6",
    })
}
pub async fn next<C: ConnectionTrait>(
    db: &C,
    user: i64,
    f: &Filters,
    direction: Direction,
) -> Result<Option<(cards::Model, &'static str)>, AppError> {
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
        if let Some(card) = query.limit(1).one(db).await? {
            let m = memory(db, user, &card, d).await?;
            candidates.push((m.as_ref().map(|m| m.due_at), card, *d));
        }
    }
    use rand::seq::SliceRandom;
    candidates.shuffle(&mut rand::thread_rng());
    candidates.sort_by_key(|(due, _, _)| (due.is_none(), *due));
    Ok(candidates.into_iter().next().map(|(_, c, d)| (c, d)))
}
