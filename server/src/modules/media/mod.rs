mod handlers;
mod service;
mod validation;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    File,
}
impl Kind {
    pub fn field(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
        }
    }
    pub fn limit(self) -> usize {
        match self {
            Self::Image => 5 * 1024 * 1024,
            Self::File => 10 * 1024 * 1024,
        }
    }
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{id}/images",
            post(handlers::image_upload).layer(DefaultBodyLimit::max(5 * 1024 * 1024 + 65536)),
        )
        .route(
            "/sessions/{id}/files",
            post(handlers::file_upload).layer(DefaultBodyLimit::max(10 * 1024 * 1024 + 65536)),
        )
        .route("/sessions/{id}/images/{media}", get(handlers::image))
        .route("/sessions/{id}/files/{media}", get(handlers::info))
        .route(
            "/sessions/{id}/files/{media}/download",
            get(handlers::download),
        )
}
