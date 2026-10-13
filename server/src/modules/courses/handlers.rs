use super::{
    dto::{CoursePage, CourseResource, CreateCourse, UpdateCourse},
    service,
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
pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(place_id): Path<i64>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<CoursePage>, AppError> {
    let Query(input) = query.map_err(|error| AppError::validation("query", &error.body_text()))?;
    Ok(Json(
        service::list(&state.db, user.id, place_id, input).await?,
    ))
}
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(place_id): Path<i64>,
    body: Result<Json<CreateCourse>, JsonRejection>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    let Json(input) = body.map_err(|error| AppError::validation("body", &error.body_text()))?;
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, place_id, input).await?,
    ))
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<UpdateCourse>, JsonRejection>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    let Json(input) = body.map_err(|error| AppError::validation("body", &error.body_text()))?;
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
