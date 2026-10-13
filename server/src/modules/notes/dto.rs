use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize)]
pub struct NoteResource {
    pub session_id: i64,
    pub content: Value,
    pub schema_version: i32,
    pub revision: i32,
    pub saved_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveNote {
    pub content: Value,
    pub schema_version: i32,
    pub base_revision: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub email: String,
}
#[derive(Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub email: String,
}
#[derive(Serialize)]
pub struct SharedNote {
    pub id: i64,
    pub title: String,
    pub note_saved_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}
pub enum SaveOutcome {
    Saved(NoteResource),
    Conflict(NoteResource),
}
