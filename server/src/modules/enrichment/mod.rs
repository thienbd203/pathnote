pub(crate) mod client;
mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{routing::post, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions/{id}/pronunciation", post(handlers::ipa))
        .route("/sessions/{id}/pronunciation/batch", post(handlers::batch))
        .route("/sessions/{id}/translation", post(handlers::translate))
}
