mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{routing::get, Router};
pub(super) use service::{filtered, scope, within_course};
pub fn routes() -> Router<AppState> {
    Router::new().route("/vocabulary", get(handlers::list))
}
