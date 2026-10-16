mod dto;
mod handlers;
mod phonemes;
mod service;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ipa-vocabulary", get(handlers::words))
        .route("/ipa-pairs", get(handlers::pairs))
        .route("/ipa-vocabulary/import", post(handlers::import_words))
        .route("/ipa-pairs/import", post(handlers::import_pairs))
        .layer(DefaultBodyLimit::max(262144))
}
