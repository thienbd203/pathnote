use super::dto::HealthStatus;
use crate::AppError;
pub async fn health(db: &sea_orm::DatabaseConnection) -> Result<HealthStatus, AppError> {
    db.ping().await?;
    Ok(HealthStatus {
        status: "ok",
        database: "ok",
        version: option_env!("PATHNOTE_BUILD_SHA").unwrap_or("development"),
    })
}
