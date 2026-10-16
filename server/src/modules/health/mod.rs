mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new().route("/health", get(handlers::health))
}
