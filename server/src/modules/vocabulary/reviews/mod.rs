mod answer;
mod create;
mod dto;
pub(super) mod state;
use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Path, Query, State,
    },
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/vocabulary-reviews", post(start))
        .route("/vocabulary-reviews/{id}", get(show))
        .route("/vocabulary-reviews/{id}/answers", post(submit))
        .route("/vocabulary-reviews/{id}/pronunciation", get(pronunciation))
}
async fn start(
    State(s): State<AppState>,
    user: CurrentUser,
    body: Result<Json<dto::Create>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(input) = body.map_err(|_| AppError::validation("mode", "Lượt ôn không hợp lệ."))?;
    Ok(ApiResponse::created(
        create::create(&s.db, user.id, input).await?.state(),
    ))
}
async fn show(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        state::load(s.db.as_ref(), user.id, id).await?.state(),
    ))
}
async fn submit(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<dto::AnswerInput>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(input) = body.map_err(|_| AppError::validation("answer", "Đáp án không hợp lệ."))?;
    Ok(ApiResponse::ok(
        answer::answer(&s.db, user.id, id, input).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    position: usize,
}
async fn pronunciation(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    query: Result<Query<Position>, QueryRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Query(input) =
        query.map_err(|_| AppError::validation("position", "Vị trí không hợp lệ."))?;
    let review = state::load(s.db.as_ref(), user.id, id).await?;
    if review.mode != dto::Mode::Typing || input.position != review.answers.len() {
        return Err(AppError::Conflict("Câu hỏi đã thay đổi.".into()));
    }
    let q = review
        .questions
        .get(input.position)
        .ok_or_else(|| AppError::Conflict("Lượt ôn đã hoàn thành.".into()))?;
    Ok(ApiResponse::ok(
        json!({"text":q.card.word,"locale":q.card.locale}),
    ))
}
