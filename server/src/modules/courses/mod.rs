mod dto;
mod handlers;
pub(super) mod service;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/courses/bulk", get(handlers::read_many))
        .route("/courses/library-stats", get(handlers::library_summaries))
        .route("/courses", axum::routing::delete(super::bulk::courses))
        .route("/courses/search", get(handlers::search))
        .route("/courses/{id}/library-stats", get(handlers::library_stats))
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
