use super::{dto::InitialNote, service};
use crate::modules::notes::document::builders::{self, paragraph, structure};
use crate::{entities::notes, modules::notes::document, AppError};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::Value;

pub async fn prepare(
    db: &DatabaseConnection,
    user: i64,
    course_id: Option<i64>,
    input: &InitialNote,
) -> Result<Option<Value>, AppError> {
    let content = match input {
        InitialNote::Blank => return Ok(None),
        InitialNote::LessonTemplate => builders::lesson_template(),
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
                    builders::from_nodes(content)
                }
                None => builders::empty(),
            }
        }
    };
    document::validate(&content, 3)?;
    Ok(Some(content))
}
