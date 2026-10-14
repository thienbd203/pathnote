pub(crate) mod cleanup;
pub(in crate::modules) mod service;
use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, State},
    routing::post,
    Json, Router,
};
use serde_json::Value;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions/{session}/ai-study/imports", post(prepare))
        .layer(DefaultBodyLimit::max(262144))
}
async fn prepare(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(session): Path<i64>,
    body: Result<Json<service::Input>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(input) = body.map_err(|_| {
        AppError::validation("content", "Nội dung Học cùng AI không hợp lệ hoặc quá lớn.")
    })?;
    Ok(ApiResponse::ok(
        service::prepare(&s.db, u.id, session, input).await?,
    ))
}
