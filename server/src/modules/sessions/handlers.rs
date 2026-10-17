use super::{
    dto::{CreateSession, SessionPage, SessionResource, UpdateSession},
    service::{self, Parent},
};
use crate::extract::{AppJson, AppPath, AppQuery};
use crate::{
    access_token::CurrentUser, pagination::ListQuery, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{extract::State, http::StatusCode, Json};

async fn list(
    state: AppState,
    user: CurrentUser,
    parent: Parent,
    AppQuery(input): AppQuery<ListQuery>,
) -> Result<Json<SessionPage>, AppError> {
    Ok(Json(
        service::list(&state.db, user.id, parent, input).await?,
    ))
}
async fn create(
    state: AppState,
    user: CurrentUser,
    parent: Parent,
    AppJson(input): AppJson<CreateSession>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, parent, input).await?,
    ))
}
pub async fn course_list(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppQuery(input): AppQuery<ListQuery>,
) -> Result<Json<SessionPage>, AppError> {
    list(state, user, Parent::Course(id), AppQuery(input)).await
}
pub async fn place_list(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppQuery(input): AppQuery<ListQuery>,
) -> Result<Json<SessionPage>, AppError> {
    list(state, user, Parent::Place(id), AppQuery(input)).await
}
pub async fn course_create(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<CreateSession>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    create(state, user, Parent::Course(id), AppJson(input)).await
}
pub async fn place_create(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<CreateSession>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    create(state, user, Parent::Place(id), AppJson(input)).await
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<SessionResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn neighbors(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<service::Neighbors>, AppError> {
    Ok(ApiResponse::ok(
        service::neighbors(state.db.as_ref(), user.id, id).await?,
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<UpdateSession>,
) -> Result<ApiResponse<SessionResource>, AppError> {
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

pub async fn search(
    State(state): State<AppState>,
    user: CurrentUser,
    // Preserve the search endpoint's existing validation message.
    input: Result<AppQuery<super::dto::SearchQuery>, AppError>,
) -> Result<Json<SessionPage>, AppError> {
    let AppQuery(input) =
        input.map_err(|_| AppError::validation("query", "Tham số tìm kiếm không hợp lệ."))?;
    Ok(Json(service::search(&state.db, user.id, input).await?))
}

pub async fn read_many(
    State(state): State<AppState>,
    user: CurrentUser,
    AppQuery(input): AppQuery<crate::pagination::IdsQuery>,
) -> Result<ApiResponse<Vec<SessionResource>>, AppError> {
    Ok(ApiResponse::ok(
        service::read_many(&state.db, user.id, input).await?,
    ))
}
