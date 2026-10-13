use crate::{
    entities::{
        courses, places,
        sessions::{self, SessionStatus},
    },
    pagination::Page,
    AppError,
};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSession {
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub duration_minutes: i32,
    pub timezone: String,
    pub status: Option<SessionStatus>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSession {
    #[serde(default, with = "serde_with::rust::double_option")]
    pub title: Option<Option<String>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub starts_at: Option<Option<DateTime<Utc>>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub duration_minutes: Option<Option<i32>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub status: Option<Option<SessionStatus>>,
    pub timezone: Option<String>,
}
pub fn title(value: String) -> Result<String, AppError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 200 {
        return Err(AppError::validation(
            "title",
            "Tên buổi học cần từ 1 đến 200 ký tự.",
        ));
    }
    Ok(value)
}
pub fn timezone(value: &str) -> Result<Tz, AppError> {
    value
        .parse()
        .map_err(|_| AppError::validation("timezone", "Múi giờ không hợp lệ."))
}
pub fn time(start: DateTime<Utc>, minutes: i32, tz: Tz) -> Result<(), AppError> {
    if !(1..=1440).contains(&minutes) {
        return Err(AppError::validation(
            "duration_minutes",
            "Thời lượng từ 1 đến 1440 phút.",
        ));
    }
    if !start.timestamp_subsec_nanos().is_multiple_of(1000) {
        return Err(AppError::validation(
            "starts_at",
            "Độ chính xác thời gian tối đa microsecond.",
        ));
    }
    let end = start
        .checked_add_signed(Duration::minutes(minutes as i64))
        .ok_or_else(|| AppError::validation("starts_at", "Thời điểm vượt giới hạn."))?;
    if start.with_timezone(&tz).date_naive() != end.with_timezone(&tz).date_naive() {
        return Err(AppError::validation(
            "duration_minutes",
            "Buổi học phải kết thúc trong cùng ngày học.",
        ));
    }
    Ok(())
}
#[derive(Serialize)]
pub struct ParentSummary {
    pub id: i64,
    pub name: String,
}
#[derive(Serialize)]
pub struct SessionResource {
    pub id: i64,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub duration_minutes: i16,
    pub status: SessionStatus,
    pub place: ParentSummary,
    pub course: Option<ParentSummary>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}
impl SessionResource {
    pub fn new(
        session: sessions::Model,
        place: places::Model,
        course: Option<courses::Model>,
    ) -> Self {
        let start = session.starts_at.with_timezone(&Utc);
        Self {
            id: session.id,
            title: session.title,
            starts_at: start,
            ends_at: start + Duration::minutes(session.duration_minutes as i64),
            duration_minutes: session.duration_minutes,
            status: session.status,
            place: ParentSummary {
                id: place.id,
                name: place.name,
            },
            course: course.map(|c| ParentSummary {
                id: c.id,
                name: c.name,
            }),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}
pub type SessionPage = Page<SessionResource>;
