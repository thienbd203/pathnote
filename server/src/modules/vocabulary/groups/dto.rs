use crate::AppError;
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGroup {
    pub name: String,
    pub note: String,
    pub card_ids: Vec<i64>,
    pub revision: Option<i32>,
}
impl SaveGroup {
    pub fn validate(&mut self) -> Result<(), AppError> {
        self.name = crate::validation::required_text(
            std::mem::take(&mut self.name),
            "name",
            100,
            "Tên nhóm cần từ 1 đến 100 ký tự.",
        )?;
        if self.note.chars().count() > 6000 {
            return Err(AppError::validation("note", "Ghi chú tối đa 6000 ký tự."));
        }
        let unique: std::collections::HashSet<_> = self.card_ids.iter().collect();
        if self.card_ids.len() > 100
            || unique.len() != self.card_ids.len()
            || self.card_ids.iter().any(|id| *id <= 0)
        {
            return Err(AppError::validation(
                "card_ids",
                "Chọn tối đa 100 mục từ khác nhau.",
            ));
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub q: String,
    #[serde(default = "one")]
    pub page: u64,
}
fn one() -> u64 {
    1
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delete {
    pub revision: i32,
}
#[derive(Serialize)]
pub struct SummaryResource {
    pub id: i64,
    pub name: String,
    pub count: usize,
    pub revision: i32,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
#[derive(Serialize)]
pub struct GroupResource {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub revision: i32,
    pub members: Vec<MemberResource>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
#[derive(Serialize)]
pub struct MemberResource {
    pub card_id: i64,
    pub card: Option<CardResource>,
}
#[derive(Serialize)]
pub struct CardResource {
    pub id: i64,
    pub word: String,
    pub ipa: String,
    pub locale: String,
    pub part_of_speech: String,
    pub meaning: String,
    pub examples: Vec<ExampleResource>,
    pub source: SourceResource,
}
#[derive(Serialize, Deserialize)]
pub struct ExampleResource {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub ipa: String,
    #[serde(default)]
    pub meaning: String,
}
#[derive(Serialize)]
pub struct SourceResource {
    pub id: i64,
    pub title: String,
}
