pub(crate) mod cleanup;
mod dto;
mod handlers;
pub(in crate::modules) mod service;
use crate::state::AppState;
use axum::{extract::DefaultBodyLimit, routing::post, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{session}/ai-study/imports",
            post(handlers::prepare),
        )
        .layer(DefaultBodyLimit::max(262144))
}
