use axum::{extract::State, Json};

use super::{
    dto::{DevLoginRequest, TokenResponse, UserResource},
    service,
};
use crate::{auth::CurrentUser, error::AppError, response::ApiResponse, state::AppState};

pub async fn dev_login(
    State(state): State<AppState>,
    Json(payload): Json<DevLoginRequest>,
) -> Result<ApiResponse<TokenResponse>, AppError> {
    let email = payload.email.trim().to_lowercase();

    if !email.contains('@') || email.len() > 255 {
        return Err(AppError::BadRequest("Email không hợp lệ".to_string()));
    }

    let user = service::find_or_create_by_email(&state.db, &email, payload.name).await?;
    let access_token = state.jwt.issue_access_token(user.id)?;

    Ok(ApiResponse::ok(TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: state.jwt.access_ttl_secs(),
        user: user.into(),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<UserResource>, AppError> {
    let row = service::find_by_id(&state.db, user.id).await?;
    Ok(ApiResponse::ok(row.into()))
}
