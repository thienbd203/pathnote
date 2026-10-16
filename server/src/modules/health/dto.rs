use serde::Serialize;
#[derive(Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub database: &'static str,
    pub version: &'static str,
}
