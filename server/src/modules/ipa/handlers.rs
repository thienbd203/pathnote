use super::{dto::*, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppQuery},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{extract::State, Json};
pub async fn words(
    State(state): State<AppState>,
    user: CurrentUser,
    q: Result<AppQuery<Query>, AppError>,
) -> Result<Json<ListResource>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppQuery(q) = q.map_err(|_| invalid())?;
    Ok(Json(service::list(&state.db, user.id, q, "word").await?))
}
pub async fn pairs(
    State(state): State<AppState>,
    user: CurrentUser,
    q: Result<AppQuery<Query>, AppError>,
) -> Result<Json<ListResource>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppQuery(q) = q.map_err(|_| invalid())?;
    Ok(Json(service::list(&state.db, user.id, q, "pair").await?))
}
pub async fn import_words(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<Words>, AppError>,
) -> Result<ApiResponse<ImportResource>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppJson(input) = input.map_err(|_| invalid())?;
    Ok(ApiResponse::ok(
        service::import_words(&state.db, user.id, input).await?,
    ))
}
pub async fn import_pairs(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<Pairs>, AppError>,
) -> Result<ApiResponse<ImportResource>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppJson(input) = input.map_err(|_| invalid())?;
    Ok(ApiResponse::ok(
        service::import_pairs(&state.db, user.id, input).await?,
    ))
}

pub async fn words_duplicates(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<Words>, AppError>,
) -> Result<ApiResponse<Duplicates>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppJson(input) = input.map_err(|_| invalid())?;
    Ok(ApiResponse::ok(
        service::words_duplicates(&state.db, user.id, input).await?,
    ))
}

pub async fn pairs_duplicates(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<Pairs>, AppError>,
) -> Result<ApiResponse<Duplicates>, AppError> {
    // Giữ lỗi entries của contract IPA thay cho body/query chung của extractor.
    let AppJson(input) = input.map_err(|_| invalid())?;
    Ok(ApiResponse::ok(
        service::pairs_duplicates(&state.db, user.id, input).await?,
    ))
}
