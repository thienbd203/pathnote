use crate::{
    entities::{courses, places},
    pagination::Page,
    AppError,
};
use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCourse {
    pub name: String,
    pub description: Option<String>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCourse {
    #[serde(default, with = "serde_with::rust::double_option")]
    pub name: Option<Option<String>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub starts_on: Option<Option<NaiveDate>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub ends_on: Option<Option<NaiveDate>>,
}
pub fn validate_name(name: String) -> Result<String, AppError> {
    crate::validation::required_text(name, "name", 200, "Tên khóa học cần từ 1 đến 200 ký tự.")
}
pub fn validate_description(value: Option<String>) -> Result<Option<String>, AppError> {
    crate::validation::optional_text(value, "description", 2000, "Mô tả tối đa 2000 ký tự.")
}
pub fn validate_dates(start: NaiveDate, end: Option<NaiveDate>) -> Result<(), AppError> {
    if end.is_some_and(|end| end < start) {
        return Err(AppError::validation(
            "ends_on",
            "Ngày kết thúc phải bằng hoặc sau ngày bắt đầu.",
        ));
    }
    Ok(())
}
#[derive(Serialize)]
pub struct PlaceSummary {
    pub id: i64,
    pub name: String,
}
#[derive(Serialize)]
pub struct CourseResource {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub place: PlaceSummary,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}
impl CourseResource {
    pub fn new(course: courses::Model, place: places::Model) -> Self {
        Self {
            id: course.id,
            name: course.name,
            description: course.description,
            starts_on: course.starts_on,
            ends_on: course.ends_on,
            place: PlaceSummary {
                id: place.id,
                name: place.name,
            },
            created_at: course.created_at,
            updated_at: course.updated_at,
        }
    }
}
pub type CoursePage = Page<CourseResource>;

#[derive(sea_orm::FromQueryResult, Serialize)]
pub struct SessionStats {
    #[serde(skip_serializing)]
    pub(super) title: String,
    #[serde(skip_serializing)]
    pub(super) starts_at: chrono::DateTime<chrono::FixedOffset>,
    pub(super) id: i64,
    pub(super) words: i64,
    pub(super) exercises: i64,
}
#[derive(Serialize)]
pub struct CourseStats {
    pub(super) session_count: usize,
    pub(super) latest_past_session: Option<LatestSession>,
    pub(super) sessions: Vec<SessionStats>,
    pub(super) words: i64,
    pub(super) exercises: i64,
}

#[derive(Serialize)]
pub struct LatestSession {
    pub(super) id: i64,
    pub(super) title: String,
    pub(super) starts_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct CourseSummary {
    pub id: i64,
    pub session_count: i64,
    pub words: i64,
    pub exercises: i64,
    pub latest_past_session: Option<LatestSession>,
}
#[derive(sea_orm::FromQueryResult)]
pub(super) struct SummaryRow {
    pub id: i64,
    pub session_count: i64,
    pub words: i64,
    pub exercises: i64,
    pub latest_id: Option<i64>,
    pub latest_title: Option<String>,
    pub latest_starts_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}
impl From<SummaryRow> for CourseSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            session_count: row.session_count,
            words: row.words,
            exercises: row.exercises,
            latest_past_session: row
                .latest_id
                .zip(row.latest_title)
                .zip(row.latest_starts_at)
                .map(|((id, title), starts_at)| LatestSession {
                    id,
                    title,
                    starts_at: starts_at.with_timezone(&chrono::Utc),
                }),
        }
    }
}
