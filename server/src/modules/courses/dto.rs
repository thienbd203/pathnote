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
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(AppError::validation(
            "name",
            "Tên khóa học cần từ 1 đến 200 ký tự.",
        ));
    }
    Ok(name)
}
pub fn validate_description(value: Option<String>) -> Result<Option<String>, AppError> {
    value
        .map(|v| {
            let v = v.trim().to_owned();
            if v.chars().count() > 2000 {
                Err(AppError::validation(
                    "description",
                    "Mô tả tối đa 2000 ký tự.",
                ))
            } else {
                Ok(v)
            }
        })
        .transpose()
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
