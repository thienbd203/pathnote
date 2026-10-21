use super::{dto, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath, AppQuery},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use subtle::ConstantTimeEq;
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/sessions/{id}/collaboration/authorize", post(authorize))
        .route("/internal/notes/{id}", get(fetch).put(store))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state, bridge_secret))
}
async fn bridge_secret(
    State(s): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let expected = s
        .config
        .collaboration_secret
        .as_deref()
        .filter(|v| v.len() >= 32)
        .ok_or(AppError::Forbidden)?;
    if s.config.collaboration_url.is_none() {
        return Err(AppError::Forbidden);
    }
    let provided = request
        .headers()
        .get("x-collaboration-secret")
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    if !bool::from(expected.as_bytes().ct_eq(provided)) {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}

async fn authorize(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppQuery(client): AppQuery<dto::ClientSchema>,
) -> Result<ApiResponse<dto::CollaborationUserResource>, AppError> {
    Ok(ApiResponse::ok(
        service::collaboration::authorize(&s.db, user.id, id, client.schema_version).await?,
    ))
}
async fn fetch(
    State(s): State<AppState>,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<dto::StateResource>, AppError> {
    Ok(ApiResponse::ok(
        service::collaboration::fetch(&s.db, id).await?,
    ))
}
async fn store(
    State(s): State<AppState>,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<dto::SaveState>,
) -> Result<ApiResponse<dto::NoteResource>, AppError> {
    Ok(ApiResponse::ok(
        service::collaboration::store(&s.db, id, input).await?,
    ))
}
