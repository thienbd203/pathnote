use crate::entities::media;
use serde::Serialize;
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
            Self::File => 10 * 1024 * 1024,
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
