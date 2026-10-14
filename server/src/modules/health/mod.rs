use axum::{extract::State, routing::get, Router};
use serde::Serialize;

use crate::{error::AppError, response::ApiResponse, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

#[derive(Serialize)]
struct HealthStatus {
    status: &'static str,
    database: &'static str,
    version: &'static str,
}

async fn health(State(state): State<AppState>) -> Result<ApiResponse<HealthStatus>, AppError> {
    state.db.ping().await?;

    Ok(ApiResponse::ok(HealthStatus {
        status: "ok",
        database: "ok",
        version: option_env!("PATHNOTE_BUILD_SHA").unwrap_or("development"),
    }))
}
