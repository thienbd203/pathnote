mod dto;
mod handlers;
pub(super) mod service;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/courses", axum::routing::delete(super::bulk::courses))
        .route(
            "/places/{place_id}/courses",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/courses/{id}",
            get(handlers::show)
                .patch(handlers::update)
                .delete(handlers::delete),
        )
}
