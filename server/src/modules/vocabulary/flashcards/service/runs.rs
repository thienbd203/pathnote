use super::super::{
    dto::{Configuration, Filters, RunSummary},
    queue,
};
use crate::{
    entities::{flashcard_attempts as attempts, flashcard_runs as runs},
    AppError,
};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QuerySelect, Set,
};

use uuid::Uuid;
pub async fn load<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: Uuid,
) -> Result<runs::Model, AppError> {
    runs::Entity::find_by_id(id)
        .filter(runs::Column::UserId.eq(user))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy lượt ôn.".into()))
}
pub async fn summary<C: ConnectionTrait>(
    db: &C,
    run: &runs::Model,
) -> Result<RunSummary, AppError> {
    let base = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(run.user_id))
        .filter(attempts::Column::RunId.eq(run.id))
        .filter(attempts::Column::Rating.is_not_null());
    // One aggregate over the user's graded tickets, including distinct study groups.
    let counts = base
        .select_only()
        .expr(Expr::cust("COUNT(*)"))
        .expr(Expr::cust("COUNT(DISTINCT (study_key, direction))"))
        .expr(Expr::cust("COUNT(*) FILTER (WHERE rating = 1)"))
        .expr(Expr::cust("COUNT(*) FILTER (WHERE rating = 2)"))
        .expr(Expr::cust("COUNT(*) FILTER (WHERE rating = 3)"))
        .expr(Expr::cust("COUNT(*) FILTER (WHERE rating = 4)"))
        .into_tuple::<(i64, i64, i64, i64, i64, i64)>()
        .one(db)
        .await?
        .unwrap_or_default();
    let reviewed = counts.0 as u64;
    let unique = counts.1;
    let ratings = vec![
        counts.2 as u64,
        counts.3 as u64,
        counts.4 as u64,
        counts.5 as u64,
    ];
    Ok(RunSummary {
        id: run.id,
        filters: serde_json::from_value(run.filters.clone())
            .map_err(|_| AppError::Internal("Invalid saved flashcard filters".into()))?,
        settings: Configuration::from_saved(run.settings.clone())?,
        initial_total: run.initial_total,
        reviewed,
        unique_reviewed: unique,
        ratings,
        started_at: run.started_at,
        completed: run.completed,
        finished_at: run.finished_at,
        elapsed_seconds: (run
            .finished_at
            .unwrap_or_else(|| chrono::Utc::now().fixed_offset())
            - run.started_at)
            .num_seconds()
            .max(0),
    })
}
pub async fn finish<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: Uuid,
    completed: bool,
) -> Result<(), AppError> {
    runs::Entity::update_many()
        .col_expr(
            runs::Column::FinishedAt,
            Expr::value(chrono::Utc::now().fixed_offset()),
        )
        .col_expr(runs::Column::Completed, Expr::value(completed))
        .filter(runs::Column::UserId.eq(user))
        .filter(runs::Column::Id.eq(id))
        .filter(runs::Column::FinishedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}
pub async fn create(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    mut filters: Filters,
) -> Result<RunSummary, AppError> {
    filters.validate()?;
    filters.run_id = None;
    queue::authorize(db, user, &filters).await?;
    let overview = queue::overview(db, user, &filters).await?;
    let total = (overview.new + if filters.new_only { 0 } else { overview.due }) as i64;
    let run =
        runs::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user),
            filters: Set(
                serde_json::to_value(&filters).map_err(|e| AppError::Internal(e.to_string()))?
            ),
            settings: Set(serde_json::to_value(&overview.settings)
                .map_err(|e| AppError::Internal(e.to_string()))?),
            initial_total: Set(total),
            started_at: Set(chrono::Utc::now().fixed_offset()),
            completed: Set(false),
            finished_at: Set(None),
        }
        .insert(db)
        .await?;
    summary(db, &run).await
}
pub async fn show(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    id: Uuid,
) -> Result<RunSummary, AppError> {
    let run = load(db, user, id).await?;
    summary(db, &run).await
}
pub async fn end(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    id: Uuid,
) -> Result<RunSummary, AppError> {
    load(db, user, id).await?;
    finish(db, user, id, false).await?;
    let run = load(db, user, id).await?;
    summary(db, &run).await
}

pub async fn optional_summary<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: Option<Uuid>,
) -> Result<Option<RunSummary>, AppError> {
    match id {
        Some(id) => {
            let run = load(db, user, id).await?;
            Ok(Some(summary(db, &run).await?))
        }
        None => Ok(None),
    }
}
