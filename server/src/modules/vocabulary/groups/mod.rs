mod dto;
mod handlers;
mod service;
use crate::state::AppState;
use axum::{
    routing::{get, put},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/word-groups", get(handlers::list).post(handlers::create))
        .route("/word-groups/search", get(handlers::search))
        .route(
            "/word-groups/{id}",
            get(handlers::show)
                .merge(put(handlers::update))
                .delete(handlers::delete),
        )
}
