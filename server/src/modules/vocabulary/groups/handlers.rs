use super::{dto::*, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath, AppQuery},
    pagination::Page,
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{extract::State, http::StatusCode, Json};
pub async fn list(
    State(s): State<AppState>,
    u: CurrentUser,
) -> Result<ApiResponse<Vec<SummaryResource>>, AppError> {
    Ok(ApiResponse::ok(service::list(&s.db, u.id).await?))
}
pub async fn show(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<GroupResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(s.db.as_ref(), u.id, id).await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    AppJson(input): AppJson<SaveGroup>,
) -> Result<ApiResponse<GroupResource>, AppError> {
    Ok(ApiResponse::created(
        service::save(&s.db, u.id, None, input).await?,
    ))
}
pub async fn update(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<SaveGroup>,
) -> Result<ApiResponse<GroupResource>, AppError> {
    Ok(ApiResponse::ok(
        service::save(&s.db, u.id, Some(id), input).await?,
    ))
}
pub async fn delete(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppQuery(input): AppQuery<Delete>,
) -> Result<StatusCode, AppError> {
    service::delete(&s.db, u.id, id, input.revision).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn search(
    State(s): State<AppState>,
    u: CurrentUser,
    AppQuery(input): AppQuery<Search>,
) -> Result<Json<Page<CardResource>>, AppError> {
    Ok(Json(service::search(&s.db, u.id, input).await?))
}
