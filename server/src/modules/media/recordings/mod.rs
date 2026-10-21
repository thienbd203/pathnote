mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions/{id}/recordings", get(handlers::list))
        .route(
            "/sessions/{id}/recordings/{media}/playback",
            post(handlers::playback),
        )
        .route(
            "/sessions/{id}/recordings/{media}/stream",
            get(handlers::stream),
        )
        .route(
            "/sessions/{id}/recordings/{media}/progress",
            get(handlers::read_progress).put(handlers::progress),
        )
}
