use super::{
    dto::{CreateSession, SessionPage, SessionResource, UpdateSession},
    service::{self, Parent},
};
use crate::{
    access_token::CurrentUser, pagination::ListQuery, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Path, Query, State,
    },
    http::StatusCode,
    Json,
};
async fn list(
    state: AppState,
    user: CurrentUser,
    parent: Parent,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<SessionPage>, AppError> {
    let Query(input) = query.map_err(|e| AppError::validation("query", &e.body_text()))?;
    Ok(Json(
        service::list(&state.db, user.id, parent, input).await?,
    ))
}
async fn create(
    state: AppState,
    user: CurrentUser,
    parent: Parent,
    body: Result<Json<CreateSession>, JsonRejection>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    let Json(input) = body.map_err(|e| AppError::validation("body", &e.body_text()))?;
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, parent, input).await?,
    ))
}
pub async fn course_list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<SessionPage>, AppError> {
    list(state, user, Parent::Course(id), query).await
}
pub async fn place_list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<SessionPage>, AppError> {
    list(state, user, Parent::Place(id), query).await
}
pub async fn course_create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<CreateSession>, JsonRejection>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    create(state, user, Parent::Course(id), body).await
}
pub async fn place_create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<CreateSession>, JsonRejection>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    create(state, user, Parent::Place(id), body).await
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn neighbors(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<service::Neighbors>, AppError> {
    Ok(ApiResponse::ok(
        service::neighbors(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<UpdateSession>, JsonRejection>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    let Json(input) = body.map_err(|e| AppError::validation("body", &e.body_text()))?;
    Ok(ApiResponse::ok(
        service::update(&state.db, user.id, id, input).await?,
    ))
}
pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    service::delete(&state.db, user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
