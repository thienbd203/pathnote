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
    /// This is a streaming memory cap, not format validation: bytes are still inspected.
    pub fn upload_limit(self, name: &str) -> usize {
        let ext = name.rsplit('.').next().unwrap_or("");
        if self == Self::File
            && !["mp3", "m4a", "mp4"]
                .iter()
                .any(|recording| ext.eq_ignore_ascii_case(recording))
        {
            10 * 1024 * 1024
        } else {
            self.limit()
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
    super::filename::validate_rename(name, original)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_limits_are_selected_before_buffering_and_do_not_trust_mime_headers() {
        for name in [
            "file.pdf",
            "file.txt",
            "file.docx",
            "unknown",
            "audio.mp3.txt",
            "audio.mp3 ",
        ] {
            assert_eq!(Kind::File.upload_limit(name), 10 * 1024 * 1024);
        }
        for name in ["audio.MP3", "audio.M4A", "video.Mp4"] {
            assert_eq!(Kind::File.upload_limit(name), 100 * 1024 * 1024);
            assert_eq!(Kind::Image.upload_limit(name), 5 * 1024 * 1024);
        }
    }
}
