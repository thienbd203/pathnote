use super::dto::Kind;
use crate::AppError;
fn forbidden(c: char) -> bool {
    c.is_control() || ['/', '\\', '"'].contains(&c)
}
pub(super) fn sanitize_upload(name: String, kind: Kind) -> String {
    let mut name = name
        .chars()
        .map(|c| if forbidden(c) { '_' } else { c })
        .collect::<String>();
    if name.chars().count() > 200 {
        if kind == Kind::File {
            let ext = name.rsplit('.').next().unwrap_or("").to_owned();
            let prefix = name
                .chars()
                .take(200usize.saturating_sub(ext.chars().count() + 1))
                .collect::<String>();
            name = format!("{prefix}.{ext}");
        } else {
            name = name.chars().take(200).collect();
        }
    }
    name
}
pub(super) fn validate_rename(name: String, original: &str) -> Result<String, AppError> {
    let name = name.trim().to_owned();
    let original_ext = original.rsplit_once('.').map(|(_, ext)| ext);
    let new_parts = name.rsplit_once('.');
    if name.chars().count() > 200
        || name.chars().any(forbidden)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_sanitizes_but_rename_rejects_unsafe_names_and_extensions_are_preserved() {
        assert_eq!(sanitize_upload("a/b\n.mp3".into(), Kind::File), "a_b_.mp3");
        let name = sanitize_upload(format!("{}.m4a", "ế".repeat(210)), Kind::File);
        assert_eq!(name.chars().count(), 200);
        assert!(name.ends_with(".m4a"));
        assert!(validate_rename("a/b.mp3".into(), "old.mp3").is_err());
        assert!(validate_rename("new.mp4".into(), "old.mp3").is_err());
        assert_eq!(
            validate_rename(" new.MP3 ".into(), "old.mp3").unwrap(),
            "new.MP3"
        );
    }
}
