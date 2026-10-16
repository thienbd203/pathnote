use super::{dto::HealthStatus, service};
use crate::{response::ApiResponse, state::AppState, AppError};
use axum::extract::State;
pub async fn health(State(state): State<AppState>) -> Result<ApiResponse<HealthStatus>, AppError> {
    Ok(ApiResponse::ok(service::health(&state.db).await?))
}
