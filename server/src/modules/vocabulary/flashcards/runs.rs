use super::{dto::Filters, queue};
use crate::{
    access_token::CurrentUser,
    entities::{flashcard_attempts as attempts, flashcard_runs as runs},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    Json,
};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect, Set,
};
use serde_json::{json, Value};
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
pub async fn summary<C: ConnectionTrait>(db: &C, run: &runs::Model) -> Result<Value, AppError> {
    let base = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(run.user_id))
        .filter(attempts::Column::RunId.eq(run.id))
        .filter(attempts::Column::Rating.is_not_null());
    let reviewed = base.clone().count(db).await?;
    let unique: i64 = base
        .clone()
        .select_only()
        .expr(Expr::cust("COUNT(DISTINCT (card_id, direction))"))
        .into_tuple()
        .one(db)
        .await?
        .unwrap_or(0);
    let mut ratings = Vec::new();
    for rating in 1..=4 {
        ratings.push(
            base.clone()
                .filter(attempts::Column::Rating.eq(rating))
                .count(db)
                .await?,
        );
    }
    Ok(
        json!({"id":run.id,"filters":run.filters,"settings":run.settings,"initial_total":run.initial_total,"reviewed":reviewed,"unique_reviewed":unique,"ratings":ratings,"started_at":run.started_at,"completed":run.completed,"finished_at":run.finished_at,"elapsed_seconds":(run.finished_at.unwrap_or_else(||chrono::Utc::now().fixed_offset())-run.started_at).num_seconds().max(0)}),
    )
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
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Json<Filters>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(mut filters) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    filters.validate()?;
    filters.run_id = None;
    queue::authorize(s.db.as_ref(), user.id, &filters).await?;
    let overview = queue::overview(s.db.as_ref(), user.id, &filters).await?;
    let total = overview["new"].as_i64().unwrap_or(0)
        + if filters.new_only {
            0
        } else {
            overview["due"].as_i64().unwrap_or(0)
        };
    let run = runs::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        filters: Set(json!(filters)),
        settings: Set(overview["settings"].clone()),
        initial_total: Set(total),
        started_at: Set(chrono::Utc::now().fixed_offset()),
        completed: Set(false),
        finished_at: Set(None),
    }
    .insert(s.db.as_ref())
    .await?;
    Ok(ApiResponse::created(summary(s.db.as_ref(), &run).await?))
}
pub async fn show(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<Value>, AppError> {
    let run = load(s.db.as_ref(), user.id, id).await?;
    Ok(ApiResponse::ok(summary(s.db.as_ref(), &run).await?))
}
pub async fn end(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<Value>, AppError> {
    load(s.db.as_ref(), user.id, id).await?;
    finish(s.db.as_ref(), user.id, id, false).await?;
    let run = load(s.db.as_ref(), user.id, id).await?;
    Ok(ApiResponse::ok(summary(s.db.as_ref(), &run).await?))
}
