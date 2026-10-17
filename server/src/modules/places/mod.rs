mod dto;
mod handlers;
pub(super) mod service;
use crate::state::AppState;
use axum::{routing::get, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/places/bulk", get(handlers::read_many))
        .route(
            "/places",
            get(handlers::list)
                .post(handlers::create)
                .delete(super::bulk::places),
        )
        .route(
            "/places/{id}",
            get(handlers::show)
                .patch(handlers::update)
                .delete(handlers::delete),
        )
}
