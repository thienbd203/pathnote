use super::{
    dto::{CreatePlace, ListPlaces, PlacePage, PlaceResource, UpdatePlace},
    service,
};
use crate::extract::{AppJson, AppPath, AppQuery};
use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::{extract::State, http::StatusCode, Json};

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    AppQuery(input): AppQuery<ListPlaces>,
) -> Result<Json<PlacePage>, AppError> {
    Ok(Json(service::list(&state.db, user.id, input).await?))
}
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    AppJson(input): AppJson<CreatePlace>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, input).await?.into(),
    ))
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(state.db.as_ref(), user.id, id).await?.into(),
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<UpdatePlace>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    Ok(ApiResponse::ok(
        service::update(&state.db, user.id, id, input).await?.into(),
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

pub async fn read_many(
    State(state): State<AppState>,
    user: CurrentUser,
    AppQuery(input): AppQuery<crate::pagination::IdsQuery>,
) -> Result<ApiResponse<Vec<super::dto::PlaceResource>>, AppError> {
    Ok(ApiResponse::ok(
        service::read_many(&state.db, user.id, input).await?,
    ))
}
