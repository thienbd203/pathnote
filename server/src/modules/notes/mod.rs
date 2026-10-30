pub(in crate::modules) mod document;
mod dto;
mod handlers;
mod internal;
mod limits;
pub(crate) mod schema;
pub(super) mod service;
use crate::state::AppState;
use axum::{
    routing::{delete, get},
    Router,
};
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(internal::routes(state))
        .route("/sessions/{id}/collaboration", get(handlers::collaboration))
        .route(
            "/sessions/{id}/note",
            get(handlers::show)
                .put(handlers::save)
                .layer(axum::extract::DefaultBodyLimit::max(
                    limits::NOTE_BODY_BYTES,
                )),
        )
        .route(
            "/sessions/{id}/collaborators",
            get(handlers::members).post(handlers::invite),
        )
        .route(
            "/sessions/{id}/collaborators/{user}",
            delete(handlers::revoke),
        )
        .route("/shared-notes", get(handlers::shared))
        .route("/shared-notes/{id}", get(handlers::metadata))
}
