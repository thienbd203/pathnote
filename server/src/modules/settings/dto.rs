use crate::AppError;
use chrono::{NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
// Store categories independently so saving learning defaults cannot overwrite
// future appearance/notification preferences. Each category has a typed API.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningSettings {
    pub session_start_time: String,
    pub session_duration_minutes: u16,
}

impl Default for LearningSettings {
    fn default() -> Self {
        Self {
            session_start_time: "19:30".into(),
            session_duration_minutes: 60,
        }
    }
}

impl LearningSettings {
    pub fn validate(&self) -> Result<(), AppError> {
        let time = NaiveTime::parse_from_str(&self.session_start_time, "%H:%M")
            .ok()
            .filter(|_| self.session_start_time.len() == 5)
            .ok_or_else(|| {
                AppError::validation(
                    "session_start_time",
                    "Giờ bắt đầu phải có dạng HH:MM từ 00:00 đến 23:59.",
                )
            })?;
        if !(1..=1440).contains(&self.session_duration_minutes) {
            return Err(AppError::validation(
                "session_duration_minutes",
                "Thời lượng phải từ 1 đến 1440 phút.",
            ));
        }
        if time.hour() * 60 + time.minute() + u32::from(self.session_duration_minutes) >= 1440 {
            return Err(AppError::validation(
                "session_duration_minutes",
                "Buổi học phải kết thúc trong cùng ngày học.",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct PersonalSettings {
    pub learning: LearningSettings,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipKind {
    Course,
    Place,
}
#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HiddenChip {
    pub kind: ChipKind,
    pub id: i64,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSettings {
    pub hidden_chips: Vec<HiddenChip>,
    pub pinned_places: Vec<i64>,
}
