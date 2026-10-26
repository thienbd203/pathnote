use super::{
    dto::{TokenResource, UserResource},
    google::{GoogleCredentials, LoginAttempt},
    service,
};
use crate::access_token::{CurrentUser, ACCESS_TOKEN_TTL};
use crate::{auth::AuthSession, error::AppError, response::ApiResponse, state::AppState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::{Expiry, Session};
const LOGIN_ATTEMPT: &str = "google_login_attempt";

#[derive(Deserialize)]
pub struct GoogleCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn google_login(
    State(state): State<AppState>,
    session: Session,
) -> Result<Redirect, AppError> {
    let (url, attempt) = state.google.authorization().await?;
    session
        .insert(LOGIN_ATTEMPT, attempt)
        .await
        .map_err(session_error)?;
    session.save().await.map_err(session_error)?;
    Ok(Redirect::to(&url))
}

pub async fn google_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    session: Session,
    Query(query): Query<GoogleCallback>,
) -> Response {
    match finish_login(&mut auth, &session, query).await {
        Ok(()) => Redirect::to(&state.config.frontend_url).into_response(),
        Err(_) => Redirect::to(&format!(
            "{}/login?error=google_login_failed",
            state.config.frontend_url
        ))
        .into_response(),
    }
}
async fn finish_login(
    auth: &mut AuthSession,
    session: &Session,
    query: GoogleCallback,
) -> Result<(), AppError> {
    let attempt: LoginAttempt = session
        .remove(LOGIN_ATTEMPT)
        .await
        .map_err(session_error)?
        .ok_or(AppError::Unauthenticated)?;
    // Consume the browser-bound attempt before exchanging the one-time Google code.
    session.save().await.map_err(session_error)?;
    let state = query
        .state
        .filter(|s| s.len() <= 1024)
        .ok_or(AppError::Unauthenticated)?;
    if query.error.is_some() || !attempt.validate_state(&state, chrono::Utc::now().timestamp()) {
        return Err(AppError::Unauthenticated);
    }
    let code = query
        .code
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(AppError::Unauthenticated)?;
    let user = auth
        .authenticate(GoogleCredentials { code, attempt })
        .await
        .map_err(|_| AppError::Unauthenticated)?
        .ok_or(AppError::Unauthenticated)?;
    // Also rotate when switching from an already authenticated account.
    if auth.user.is_some() {
        session.cycle_id().await.map_err(session_error)?;
    }
    auth.login(&user)
        .await
        .map_err(|_| AppError::Internal("Không lưu được phiên đăng nhập".into()))?;
    session.set_expiry(Some(Expiry::AtDateTime(
        time::OffsetDateTime::now_utc() + time::Duration::days(7),
    )));
    session.save().await.map_err(session_error)?;
    Ok(())
}

pub async fn refresh(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<ApiResponse<TokenResource>, AppError> {
    let user = auth.user.ok_or(AppError::Unauthenticated)?;
    Ok(ApiResponse::ok(TokenResource {
        access_token: state.access_tokens.issue(user.id)?,
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_TTL,
        user: user.into(),
    }))
}
pub async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<UserResource>, AppError> {
    Ok(ApiResponse::ok(
        service::current_user(state.db.as_ref(), user.id)
            .await?
            .into(),
    ))
}
pub async fn jwks(State(state): State<AppState>) -> axum::Json<serde_json::Value> {
    axum::Json(state.access_tokens.jwks())
}
pub async fn logout(mut auth: AuthSession) -> Result<StatusCode, AppError> {
    auth.logout()
        .await
        .map_err(|_| AppError::Internal("Không thể đăng xuất".into()))?;
    Ok(StatusCode::NO_CONTENT)
}
fn session_error(error: tower_sessions::session::Error) -> AppError {
    AppError::Internal(error.to_string())
}
