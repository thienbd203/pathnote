use super::dto::{Direction, Filters};
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
        super::super::library::scope(user, f.session_id),
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
) -> Result<Value, AppError> {
    let (direction, retention) = configuration(db, user, f).await?;
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
    Ok(
        json!({"settings":{"direction":direction,"retention":retention},"new":new,"due":due,"learning":learning,"next_due_at":next_due,"algorithm":"FSRS-6"}),
    )
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
        let mut query = base(user, f, d).filter(
            Condition::any()
                .add(new_condition())
                .add(due_condition(now)),
        );
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
