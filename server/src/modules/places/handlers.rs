use super::{
    dto::{CreatePlace, ListPlaces, PlacePage, PlaceResource, UpdatePlace},
    service,
};
use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Path, State,
    },
    http::StatusCode,
    Json,
};

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<axum::extract::Query<ListPlaces>, QueryRejection>,
) -> Result<Json<PlacePage>, AppError> {
    let axum::extract::Query(input) =
        query.map_err(|error| AppError::validation("query", &error.body_text()))?;
    Ok(Json(service::list(&state.db, user.id, input).await?))
}
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    body: Result<Json<CreatePlace>, JsonRejection>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    let Json(input) = body.map_err(|error| AppError::validation("body", &error.body_text()))?;
    Ok(ApiResponse::created(
        service::create(&state.db, user.id, input).await?.into(),
    ))
}
pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(&state.db, user.id, id).await?.into(),
    ))
}
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<UpdatePlace>, JsonRejection>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    let Json(input) = body.map_err(|error| AppError::validation("body", &error.body_text()))?;
    Ok(ApiResponse::ok(
        service::update(&state.db, user.id, id, input).await?.into(),
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
