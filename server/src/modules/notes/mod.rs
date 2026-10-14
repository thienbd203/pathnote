mod collaboration;
pub(in crate::modules) mod document;
mod dto;
mod handlers;
pub(super) mod service;
use crate::state::AppState;
use axum::{
    routing::{delete, get},
    Router,
};
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(collaboration::routes(state))
        .route(
            "/sessions/{id}/note",
            get(handlers::show).put(handlers::save),
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
