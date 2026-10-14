mod client;
mod handlers;
use crate::state::AppState;
use axum::{routing::post, Router};
pub use client::LearningServices;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions/{id}/pronunciation", post(handlers::ipa))
        .route("/sessions/{id}/pronunciation/batch", post(handlers::batch))
        .route("/sessions/{id}/translation", post(handlers::translate))
}
