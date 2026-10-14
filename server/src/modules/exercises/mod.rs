pub(in crate::modules) mod dto;
mod question;
pub(in crate::modules) mod service;
use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions/{session}/exercises", post(create))
        .route("/sessions/{session}/exercises/import", post(import))
        .route("/sessions/{session}/exercises/{id}", get(show).put(update))
        .route(
            "/sessions/{session}/exercises/{id}/definition",
            get(definition),
        )
        .route("/sessions/{session}/exercises/{id}/attempts", post(start))
        .route(
            "/sessions/{session}/exercises/{id}/attempts/{attempt}/answers",
            post(answer),
        )
        .layer(axum::extract::DefaultBodyLimit::max(262144))
}
fn body<T>(input: Result<Json<T>, JsonRejection>) -> Result<T, AppError> {
    input.map(|Json(v)| v).map_err(|_| dto::invalid())
}
async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(session): Path<i64>,
    input: Result<Json<dto::Definition>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::created(
        service::create(&s.db, u.id, session, body(input)?, false).await?,
    ))
}
async fn import(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(session): Path<i64>,
    input: Result<Json<dto::Definition>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::created(
        service::create(&s.db, u.id, session, body(input)?, true).await?,
    ))
}
async fn show(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((session, id)): Path<(i64, i64)>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        service::show(&s.db, u.id, session, id, false).await?,
    ))
}
async fn definition(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((session, id)): Path<(i64, i64)>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        service::show(&s.db, u.id, session, id, true).await?,
    ))
}
async fn update(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((session, id)): Path<(i64, i64)>,
    input: Result<Json<dto::Definition>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        service::update(&s.db, u.id, session, id, body(input)?).await?,
    ))
}
async fn start(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((session, id)): Path<(i64, i64)>,
    input: Result<Json<dto::Start>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        service::start(&s.db, u.id, session, id, body(input)?).await?,
    ))
}
async fn answer(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((session, id, attempt)): Path<(i64, i64, i64)>,
    input: Result<Json<dto::Answer>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    Ok(ApiResponse::ok(
        service::answer(&s.db, u.id, session, id, attempt, body(input)?).await?,
    ))
}
