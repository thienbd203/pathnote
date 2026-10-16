pub(crate) mod flashcards;
mod library;
mod rebuild;
mod reviews;
pub(in crate::modules) mod sync;
use crate::state::AppState;
use axum::Router;
pub use rebuild::rebuild;
pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(library::routes())
        .merge(reviews::routes())
        .merge(flashcards::routes())
}
