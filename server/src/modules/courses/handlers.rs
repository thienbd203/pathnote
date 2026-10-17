use super::{
    dto::{CoursePage, CourseResource, CreateCourse, UpdateCourse},
    service,
};
use crate::extract::{AppJson, AppPath, AppQuery};
use crate::{
    access_token::CurrentUser, pagination::ListQuery, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{extract::State, http::StatusCode, Json};

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(place_id): AppPath<i64>,
    AppQuery(input): AppQuery<ListQuery>,
) -> Result<Json<CoursePage>, AppError> {
    Ok(Json(
        service::list(&state.db, user.id, place_id, input).await?,
    ))
}
pub async fn search(
    State(state): State<AppState>,
    user: CurrentUser,
    // Preserve the existing search-specific validation message.
    query: Result<AppQuery<ListQuery>, AppError>,
) -> Result<Json<CoursePage>, AppError> {
    let AppQuery(input) =
        query.map_err(|_| AppError::validation("query", "Tham số tìm khóa học không hợp lệ."))?;
    Ok(Json(
        service::search(state.db.as_ref(), user.id, None, input).await?,
    ))
}
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(place_id): AppPath<i64>,
    AppJson(input): AppJson<CreateCourse>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, place_id, input).await?,
    ))
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<UpdateCourse>,
) -> Result<ApiResponse<CourseResource>, AppError> {
    Ok(ApiResponse::ok(
        service::update(&state.db, user.id, id, input).await?,
    ))
}
pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<StatusCode, AppError> {
    service::delete(&state.db, user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn library_stats(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<super::dto::CourseStats>, AppError> {
    Ok(ApiResponse::ok(
        service::library_stats(&state.db, user.id, id).await?,
    ))
}

pub async fn library_summaries(
    State(state): State<AppState>,
    user: CurrentUser,
    AppQuery(input): AppQuery<crate::pagination::IdsQuery>,
) -> Result<ApiResponse<Vec<super::dto::CourseSummary>>, AppError> {
    Ok(ApiResponse::ok(
        service::library_summaries(&state.db, user.id, input).await?,
    ))
}
