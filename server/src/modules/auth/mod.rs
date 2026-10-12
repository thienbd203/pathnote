mod dto;
mod handlers;
mod service;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(dev_login_enabled: bool) -> Router<AppState> {
    let router = Router::new().route("/auth/me", get(handlers::me));

    if dev_login_enabled {
        router.route("/auth/dev-login", post(handlers::dev_login))
    } else {
        router
    }
}
