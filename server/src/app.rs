use axum::{
    http::{header, HeaderValue, Method},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{config::Config, error::route_not_found, modules, state::AppState};

pub fn build_app(state: AppState, config: &Config) -> Router {
    Router::new()
        .merge(modules::health::routes())
        .fallback(route_not_found)
        .layer(cors(&config.frontend_url))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn cors(frontend_url: &str) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(HeaderValue::from_str(frontend_url).expect("FRONTEND_URL không hợp lệ"))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .allow_credentials(true)
}
