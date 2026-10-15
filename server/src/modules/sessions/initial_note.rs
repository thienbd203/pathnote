use super::{dto::InitialNote, service};
use crate::{entities::notes, modules::notes::document, AppError};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

fn paragraph() -> Value {
    json!({"type":"paragraph"})
}
fn heading(text: &str) -> Value {
    json!({"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":text}]})
}
fn vocabulary(locale: &str) -> Value {
    json!({"type":"vocabulary","attrs":{"locale":locale},"content":[{
        "type":"vocabularyEntry","attrs":{"id":uuid::Uuid::new_v4().to_string(),"word":"","ipa":"","topics":[]},"content":[{
            "type":"vocabularySense","attrs":{"id":uuid::Uuid::new_v4().to_string(),"partOfSpeech":"noun","meaning":""},"content":[{"type":"vocabularyExample","attrs":{"id":uuid::Uuid::new_v4().to_string(),"text":"","highlights":[],"ipa":null,"meaning":null,"meaningHighlights":null}}]
        }]
    }]})
}
// Copy layout and section names, never answers, file references, imported AI IDs or vocabulary IDs.
fn structure(node: &Value) -> Option<Value> {
    let kind = node["type"].as_str()?;
    match kind {
        "heading" => {
            let content = node["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|child| match child["type"].as_str() {
                    Some("text") => Some(json!({"type":"text","text":child["text"]})),
                    Some("hardBreak") => Some(json!({"type":"hardBreak"})),
                    _ => None,
                })
                .collect::<Vec<_>>();
            Some(
                json!({"type":"heading","attrs":{"level":node["attrs"]["level"]},"content":content}),
            )
        }
        "paragraph" => Some(paragraph()),
        "vocabulary" => Some(vocabulary(
            node["attrs"]["locale"].as_str().unwrap_or("en-US"),
        )),
        "codeBlock" => Some(json!({"type":"codeBlock"})),
        "horizontalRule" => Some(json!({"type":"horizontalRule"})),
        "bulletList" | "orderedList" | "taskList" | "listItem" | "taskItem" | "blockquote"
        | "table" | "tableRow" | "tableCell" | "tableHeader" => {
            let mut content = node["content"]
                .as_array()?
                .iter()
                .filter_map(structure)
                .collect::<Vec<_>>();
            if content.is_empty() {
                content.push(paragraph());
            }
            let mut result = json!({"type":kind,"content":content});
            if kind == "taskItem" {
                result["attrs"] = json!({"checked":false});
            }
            if matches!(kind, "tableCell" | "tableHeader") {
                result["attrs"] = node.get("attrs").cloned().unwrap_or(json!({}));
            }
            Some(result)
        }
        _ => None,
    }
}
pub async fn prepare(
    db: &DatabaseConnection,
    user: i64,
    course_id: Option<i64>,
    input: &InitialNote,
) -> Result<Option<Value>, AppError> {
    let content = match input {
        InitialNote::Blank => return Ok(None),
        InitialNote::LessonTemplate => {
            json!({"type":"doc","content":[heading("Ghi chú"),paragraph(),heading("Từ vựng"),vocabulary("en-US"),paragraph()]})
        }
        InitialNote::Structure { session_id } => {
            let source = service::model(db, user, *session_id).await?;
            if course_id.is_none() || source.course_id != course_id {
                return Err(AppError::validation(
                    "initial_note",
                    "Chọn buổi học trong cùng khóa để sao chép cấu trúc.",
                ));
            }
            match notes::Entity::find_by_id(*session_id).one(db).await? {
                Some(note) => {
                    document::validate(&note.content, note.schema_version)?;
                    let mut content = note.content["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(structure)
                        .collect::<Vec<_>>();
                    if content.is_empty() {
                        content.push(paragraph());
                    }
                    json!({"type":"doc","content":content})
                }
                None => json!({"type":"doc","content":[paragraph()]}),
            }
        }
    };
    document::validate(&content, 3)?;
    Ok(Some(content))
}
