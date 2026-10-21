use crate::entities::media;
use crate::AppError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    File,
}
impl Kind {
    pub fn field(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
        }
    }
    pub fn limit(self) -> usize {
        match self {
            Self::Image => 5 * 1024 * 1024,
            Self::File => 100 * 1024 * 1024,
        }
    }
}
#[derive(Serialize)]
pub struct MediaResource {
    pub id: i64,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
}
impl From<media::Model> for MediaResource {
    fn from(m: media::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            mime_type: m.mime_type,
            size: m.size,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameMedia {
    pub name: String,
}
pub fn validate_name(name: String, original: &str) -> Result<String, AppError> {
    let name = name.trim().to_owned();
    let original_ext = original.rsplit_once('.').map(|(_, ext)| ext);
    let new_parts = name.rsplit_once('.');
    if name.chars().count() > 200
        || name
            .chars()
            .any(|c| c.is_control() || ['/', '\\', '"'].contains(&c))
        || new_parts.is_none_or(|(stem, ext)| {
            stem.trim().is_empty() || !original_ext.is_some_and(|old| old.eq_ignore_ascii_case(ext))
        })
    {
        return Err(AppError::validation(
            "name",
            "Tên file cần từ 1 đến 200 ký tự, giữ nguyên định dạng và không chứa ký tự đường dẫn.",
        ));
    }
    Ok(name)
}
