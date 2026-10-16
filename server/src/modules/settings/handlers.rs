use super::{dto::*, service};
use crate::{
    access_token::CurrentUser, extract::AppJson, response::ApiResponse, state::AppState, AppError,
};
use axum::extract::State;
pub async fn read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<PersonalSettings>, AppError> {
    Ok(ApiResponse::ok(service::read(&state.db, user.id).await?))
}
pub async fn save_learning(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<LearningSettings>, AppError>,
) -> Result<ApiResponse<PersonalSettings>, AppError> {
    // Preserve the settings-specific validation message for malformed bodies.
    let AppJson(input) =
        input.map_err(|_| AppError::validation("settings", "Cài đặt học tập không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::save_learning(&state.db, user.id, input).await?,
    ))
}
pub async fn read_navigation(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<NavigationSettings>, AppError> {
    Ok(ApiResponse::ok(
        service::read_navigation(&state.db, user.id).await?,
    ))
}
pub async fn save_navigation(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<NavigationSettings>, AppError>,
) -> Result<ApiResponse<NavigationSettings>, AppError> {
    let AppJson(input) =
        input.map_err(|_| AppError::validation("navigation", "Bộ lọc cá nhân không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::save_navigation(&state.db, user.id, input).await?,
    ))
}
