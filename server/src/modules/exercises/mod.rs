pub(in crate::modules) mod dto;
mod handlers;
mod question;
pub(in crate::modules) mod service;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{session}/exercises",
            post(handlers::create).get(handlers::index),
        )
        .route(
            "/sessions/{session}/exercises/import",
            post(handlers::import),
        )
        .route(
            "/sessions/{session}/exercises/{id}",
            get(handlers::show).put(handlers::update),
        )
        .route(
            "/sessions/{session}/exercises/{id}/definition",
            get(handlers::definition),
        )
        .route(
            "/sessions/{session}/exercises/{id}/attempts",
            post(handlers::start),
        )
        .route(
            "/sessions/{session}/exercises/{id}/attempts/{attempt}/answers",
            post(handlers::answer),
        )
        .layer(axum::extract::DefaultBodyLimit::max(262144))
}
