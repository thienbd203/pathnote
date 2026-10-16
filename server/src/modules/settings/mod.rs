mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, put},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(handlers::read))
        .route("/settings/learning", put(handlers::save_learning))
        .route(
            "/settings/navigation",
            get(handlers::read_navigation)
                .put(handlers::save_navigation)
                .layer(DefaultBodyLimit::max(16384)),
        )
        .layer(DefaultBodyLimit::max(4096))
}
