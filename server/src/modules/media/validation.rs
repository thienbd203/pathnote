use super::dto::Kind;
use crate::AppError;
use std::io::{Cursor, Read};
fn invalid(kind: Kind) -> AppError {
    AppError::validation(
        kind.field(),
        "Nội dung file không hợp lệ hoặc không khớp định dạng được hỗ trợ.",
    )
}
pub fn inspect(kind: Kind, name: &str, bytes: &[u8]) -> Result<String, AppError> {
    if bytes.is_empty() || bytes.len() > kind.limit() {
        return Err(AppError::validation(
            kind.field(),
            if kind == Kind::Image {
                "Ảnh tối đa 5 MB và không được rỗng."
            } else {
                "MP3/M4A/MP4 tối đa 100 MB; tài liệu tối đa 10 MB, không được rỗng."
            },
        ));
    }
    if kind == Kind::Image {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| invalid(kind))?;
        let mime = match reader.format() {
            Some(image::ImageFormat::Png) => "image/png",
            Some(image::ImageFormat::Jpeg) => "image/jpeg",
            Some(image::ImageFormat::WebP) => "image/webp",
            _ => return Err(invalid(kind)),
        };
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().map_err(|_| invalid(kind))?;
        return Ok(mime.into());
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if ["mp3", "m4a", "mp4"].contains(&ext.as_str()) {
        let detected = infer::get(bytes).ok_or_else(|| invalid(kind))?;
        if detected.extension() != ext {
            return Err(invalid(kind));
        }
        // infer identifies the M4A brand as audio/m4a. Serve the standard MP4
        // audio media type so browsers use the audio player for this container.
        return Ok(if ext == "m4a" {
            "audio/mp4"
        } else {
            detected.mime_type()
        }
        .into());
    }
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(AppError::validation("file", "Tài liệu tối đa 10 MB."));
    }
    if ext == "txt" {
        let text = std::str::from_utf8(bytes).map_err(|_| invalid(kind))?;
        if text
            .chars()
            .any(|c| c.is_control() && !['\n', '\r', '\t'].contains(&c))
        {
            return Err(invalid(kind));
        }
        return Ok("text/plain; charset=utf-8".into());
    }
    if ["docx", "xlsx", "pptx"].contains(&ext.as_str()) {
        return office(&ext, bytes).map_err(|_| invalid(kind));
    }
    let detected = infer::get(bytes).ok_or_else(|| invalid(kind))?;
    if !["pdf", "doc", "xls", "ppt"].contains(&ext.as_str()) || detected.extension() != ext {
        return Err(invalid(kind));
    }
    Ok(detected.mime_type().into())
}
fn office(ext: &str, bytes: &[u8]) -> Result<String, ()> {
    let (part, main, mime) = match ext {
        "docx" => (
            "word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "xlsx" => (
            "xl/workbook.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        _ => (
            "ppt/presentation.xml",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
    };
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ())?;
    zip.by_name(part).map_err(|_| ())?;
    let file = zip.by_name("[Content_Types].xml").map_err(|_| ())?;
    if file.size() > 262144 {
        return Err(());
    }
    let mut xml = String::new();
    file.take(262145).read_to_string(&mut xml).map_err(|_| ())?;
    if xml.len() > 262144 {
        return Err(());
    }
    // roxmltree rejects DTD by default and never loads external entities.
    let doc = roxmltree::Document::parse(&xml).map_err(|_| ())?;
    if !doc.descendants().any(|n| {
        n.has_tag_name((
            "http://schemas.openxmlformats.org/package/2006/content-types",
            "Override",
        )) && n.attribute("PartName") == Some(format!("/{part}").as_str())
            && n.attribute("ContentType") == Some(main)
    }) {
        return Err(());
    }
    Ok(mime.into())
}
