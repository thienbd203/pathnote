use crate::AppError;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
pub struct RecordingResource {
    pub id: i64,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub position_seconds: f64,
}
#[derive(Serialize)]
pub struct PlaybackResource {
    pub ticket: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaybackQuery {
    #[serde(default)]
    pub ticket: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub position_seconds: f64,
}
impl Progress {
    pub fn validate(&self) -> Result<(), AppError> {
        if !self.position_seconds.is_finite() || !(0.0..=604800.0).contains(&self.position_seconds)
        {
            return Err(AppError::validation(
                "position_seconds",
                "Vị trí nghe không hợp lệ.",
            ));
        }
        Ok(())
    }
}
