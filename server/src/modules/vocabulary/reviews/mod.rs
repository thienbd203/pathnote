mod answer;
mod create;
mod dto;
mod handlers;
mod service;
pub(super) mod state;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/vocabulary-reviews", post(handlers::start))
        .route("/vocabulary-reviews/{id}", get(handlers::show))
        .route("/vocabulary-reviews/{id}/answers", post(handlers::submit))
        .route(
            "/vocabulary-reviews/{id}/pronunciation",
            get(handlers::pronunciation),
        )
}
