use crate::{entities::ai_imports, modules::exercises::dto::Definition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub key: uuid::Uuid,
    pub content: Value,
    pub exercises: Vec<Definition>,
}

#[derive(Serialize)]
pub struct ImportResource {
    pub key: uuid::Uuid,
    pub content: Value,
    pub expires_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}
impl From<ai_imports::Model> for ImportResource {
    fn from(model: ai_imports::Model) -> Self {
        Self {
            key: model.id,
            content: model.content["content"].clone(),
            expires_at: (!model.attached).then(|| model.created_at + super::PENDING_IMPORT_TTL),
        }
    }
}
