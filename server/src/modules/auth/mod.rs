mod dto;
pub(crate) mod google;
mod handlers;
pub(crate) mod service;
use crate::{auth::AuthSession, error::AppError, state::AppState};
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    let authenticated = Router::new()
        .route("/auth/refresh", post(handlers::refresh))
        .route_layer(axum_login::predicate_required!(
            |auth: AuthSession| async move { auth.user.is_some() },
            AppError::Unauthenticated
        ));

    Router::new()
        .merge(authenticated)
        .route("/auth/google", get(handlers::google_login))
        .route("/auth/google/callback", get(handlers::google_callback))
        .route("/auth/logout", post(handlers::logout))
}

pub fn access_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/me", get(handlers::me))
        .route("/.well-known/jwks.json", get(handlers::jwks))
}
