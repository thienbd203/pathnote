use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;
fn first() -> u64 {
    1
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    pub topic: Option<String>,
    pub q: Option<String>,
    #[serde(default = "first")]
    pub page: u64,
    pub per_page: Option<u64>,
}

#[derive(Serialize)]
pub struct Source {
    pub id: i64,
    pub title: String,
    pub shared: bool,
}
#[derive(Serialize, Default)]
pub struct Progress {
    pub correct: i32,
    pub incorrect: i32,
    pub streak: i32,
    pub due_at: Option<DateTime<FixedOffset>>,
}
#[derive(Serialize)]
pub struct Card {
    pub id: i64,
    pub word: String,
    pub word_key: String,
    pub ipa: String,
    pub locale: String,
    pub part_of_speech: String,
    pub meaning: String,
    pub topics: Value,
    pub examples: Value,
    pub source: Source,
    pub progress: Progress,
}
#[derive(Serialize)]
pub struct Meta {
    #[serde(flatten)]
    pub page: crate::pagination::PageMeta,
    pub word_count: i64,
    pub reviewable: u64,
    pub topics: Vec<String>,
    pub syncing: bool,
    pub source: Option<Source>,
}
#[derive(Serialize)]
pub struct Resource {
    pub data: Vec<Card>,
    pub meta: Meta,
}
