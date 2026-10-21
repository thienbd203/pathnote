pub(crate) mod cleanup;
mod dto;
mod handlers;
mod recordings;
mod service;
mod storage;
mod validation;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(recordings::routes())
        .route(
            "/sessions/{id}/images",
            post(handlers::image_upload).layer(DefaultBodyLimit::max(5 * 1024 * 1024 + 65536)),
        )
        .route(
            "/sessions/{id}/files",
            post(handlers::file_upload).layer(DefaultBodyLimit::max(100 * 1024 * 1024 + 65536)),
        )
        .route("/sessions/{id}/images/{media}", get(handlers::image))
        .route(
            "/sessions/{id}/files/{media}",
            get(handlers::info).patch(handlers::rename),
        )
        .route(
            "/sessions/{id}/files/{media}/download",
            get(handlers::download),
        )
}
