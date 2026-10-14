mod library;
mod rebuild;
mod reviews;
pub(in crate::modules) mod sync;
use crate::state::AppState;
use axum::{routing::get, Router};
pub use rebuild::rebuild;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/vocabulary", get(library::list))
        .merge(reviews::routes())
}
