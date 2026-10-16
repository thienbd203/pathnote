mod dto;
mod handlers;
mod initial_note;
pub(super) mod service;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions", axum::routing::delete(super::bulk::sessions))
        .route("/sessions/search", get(handlers::search))
        .route("/sessions/{id}/neighbors", get(handlers::neighbors))
        .route(
            "/courses/{id}/sessions",
            get(handlers::course_list).post(handlers::course_create),
        )
        .route(
            "/places/{id}/sessions",
            get(handlers::place_list).post(handlers::place_create),
        )
        .route(
            "/sessions/{id}",
            get(handlers::show)
                .patch(handlers::update)
                .delete(handlers::delete),
        )
}
