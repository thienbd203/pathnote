use super::dto::NoteResource;
use crate::AppError;
use serde_json::Value;

pub const NOTE_SCHEMA_VERSION: i32 = 4;
/// Only unversioned writers of persisted, pre-v4 documents use this transition.
pub const LEGACY_NOTE_SCHEMA_VERSION: i32 = 3;

pub fn check_client(version: i32) -> Result<(), AppError> {
    if version < NOTE_SCHEMA_VERSION {
        return Err(AppError::ClientOutdated);
    }
    if version > NOTE_SCHEMA_VERSION {
        return Err(AppError::validation(
            "schema_version",
            "Phiên bản ghi chép chưa hỗ trợ.",
        ));
    }
    Ok(())
}
fn has_dual_ipa(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key("ipaUK")
                || object.contains_key("ipaUS")
                || object.values().any(has_dual_ipa)
        }
        Value::Array(values) => values.iter().any(has_dual_ipa),
        _ => false,
    }
}
/// Version 4 is the single transition release for writers that omitted the field.
/// Never permit such a writer to replace a v4 document or any dual-IPA document.
/// Bumping the constant to 5 automatically ends this narrow legacy exception.
pub fn write_version(
    version: Option<i32>,
    current: &NoteResource,
    content: &Value,
) -> Result<i32, AppError> {
    match version {
        Some(value) => {
            check_client(value)?;
            if value < current.schema_version {
                return Err(AppError::ClientOutdated);
            }
            Ok(value)
        }
        None if NOTE_SCHEMA_VERSION == 4
            && current.schema_version < 4
            && !has_dual_ipa(&current.content)
            && !has_dual_ipa(content) =>
        {
            Ok(current.schema_version.max(LEGACY_NOTE_SCHEMA_VERSION))
        }
        None => Err(AppError::ClientOutdated),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn note(content: Value, schema_version: i32) -> NoteResource {
        NoteResource {
            session_id: 1,
            content,
            schema_version,
            revision: 7,
            saved_at: None,
        }
    }
    #[test]
    fn missing_version_grace_cannot_touch_already_deployed_schema_three_dual_ipa_data() {
        let protected = json!({"type":"doc","attrs":{"ipaUK":"/keep/","ipaUS":"/keep too/"}});
        let plain = json!({"type":"doc","content":[{"type":"paragraph"}]});
        assert!(matches!(
            write_version(None, &note(protected.clone(), 3), &plain),
            Err(AppError::ClientOutdated)
        ));
        assert!(matches!(
            write_version(None, &note(plain.clone(), 3), &protected),
            Err(AppError::ClientOutdated)
        ));
        assert_eq!(
            write_version(Some(4), &note(protected.clone(), 3), &protected).unwrap(),
            4
        );
    }
    #[test]
    fn grace_is_only_for_unversioned_legacy_notes_not_explicit_old_or_upgraded_clients() {
        let plain = json!({"type":"doc","content":[{"type":"paragraph"}]});
        assert_eq!(
            write_version(None, &note(plain.clone(), 3), &plain).unwrap(),
            3
        );
        assert!(matches!(
            write_version(Some(3), &note(plain.clone(), 3), &plain),
            Err(AppError::ClientOutdated)
        ));
        assert!(matches!(
            write_version(None, &note(plain.clone(), 4), &plain),
            Err(AppError::ClientOutdated)
        ));
    }
}
