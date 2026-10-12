mod dto;
pub(crate) mod google;
mod handlers;
pub(crate) mod service;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/google", get(handlers::google_login))
        .route("/auth/google/callback", get(handlers::google_callback))
        .route("/auth/refresh", post(handlers::refresh))
        .route("/auth/logout", post(handlers::logout))
}

pub fn access_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/me", get(handlers::me))
        .route("/.well-known/jwks.json", get(handlers::jwks))
}
