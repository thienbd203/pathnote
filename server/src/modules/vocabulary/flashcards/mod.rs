pub(super) const DEFAULT_RETENTION: f32 = 0.9;
mod dto;
mod handlers;
mod history;
pub(crate) mod optimization;
mod personal;
mod progress;
mod queue;
mod scheduler;
mod service;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/flashcards",
            get(handlers::overview).put(handlers::configure),
        )
        .route("/flashcards/next", post(handlers::next))
        .route("/flashcards/runs", post(handlers::create_run))
        .route("/flashcards/runs/{id}", get(handlers::show_run))
        .route("/flashcards/runs/{id}/finish", post(handlers::finish_run))
        .route("/flashcards/history", get(handlers::history))
        .route("/flashcards/statistics", get(handlers::statistics))
        .route("/flashcards/runs/{id}/time", post(handlers::record_time))
        .route("/flashcards/optimization", get(handlers::optimization))
        .route("/flashcards/{id}/reveal", post(handlers::reveal))
        .route("/flashcards/{id}/grade", post(handlers::grade))
}
