use crate::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    #[serde(default)]
    pub new_only: bool,
    pub run_id: Option<uuid::Uuid>,
    pub topic: Option<String>,
    pub q: Option<String>,
}
impl Filters {
    pub fn validate(&mut self) -> Result<(), AppError> {
        if self.course_id.is_some_and(|id| id <= 0)
            || self.session_id.is_some_and(|id| id <= 0)
            || self.topic.as_ref().is_some_and(|s| s.chars().count() > 60)
            || self.q.as_ref().is_some_and(|s| s.chars().count() > 200)
        {
            return Err(AppError::validation("filters", "Bộ lọc không hợp lệ."));
        }
        self.topic = self
            .topic
            .take()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        self.q = self
            .q
            .take()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        Ok(())
    }
    // Searching narrows the deck, but does not create another saved configuration.
    pub fn scope(&self) -> String {
        if let Some(course) = self.course_id {
            return format!(
                "{:x}",
                Sha256::digest(
                    serde_json::to_vec(&("course", course, self.session_id, &self.topic))
                        .expect("String serialization")
                )
            );
        }
        format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(self.session_id, &self.topic)).expect("String serialization")
            )
        )
    }
}
#[derive(Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    ViEn,
    EnVi,
    Both,
}
impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Self::ViEn => "vi_en",
            Self::EnVi => "en_vi",
            Self::Both => "both",
        }
    }
    pub fn directions(self) -> &'static [&'static str] {
        match self {
            Self::ViEn => &["vi_en"],
            Self::EnVi => &["en_vi"],
            Self::Both => &["vi_en", "en_vi"],
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsInput {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    pub topic: Option<String>,
    pub q: Option<String>,
    pub direction: Direction,
    pub retention: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grade {
    pub rating: u8,
}
