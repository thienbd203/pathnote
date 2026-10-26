use crate::{entities::places, AppError};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePlace {
    pub name: String,
    pub description: Option<String>,
}

// Serde distinguishes missing (None), null (Some(None)) and a supplied value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlace {
    #[serde(default, with = "serde_with::rust::double_option")]
    pub name: Option<Option<String>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub description: Option<Option<String>>,
}

pub use crate::pagination::{ListQuery as ListPlaces, PageMeta};

pub fn validate_name(name: String) -> Result<String, AppError> {
    crate::validation::required_text(name, "name", 200, "Tên nơi học cần từ 1 đến 200 ký tự.")
}
pub fn validate_description(value: Option<String>) -> Result<Option<String>, AppError> {
    crate::validation::optional_text(value, "description", 2000, "Mô tả tối đa 2000 ký tự.")
}

#[derive(Serialize)]
pub struct PlaceResource {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}
impl From<places::Model> for PlaceResource {
    fn from(place: places::Model) -> Self {
        Self {
            id: place.id,
            name: place.name,
            description: place.description,
            created_at: place.created_at,
            updated_at: place.updated_at,
        }
    }
}
pub type PlacePage = crate::pagination::Page<PlaceResource>;
