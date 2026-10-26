use crate::AppError;
pub(crate) fn required_text(
    value: String,
    field: &str,
    max: usize,
    message: &str,
) -> Result<String, AppError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > max {
        return Err(AppError::validation(field, message));
    }
    Ok(value)
}
pub(crate) fn optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
    message: &str,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            let value = value.trim().to_owned();
            if value.chars().count() > max {
                Err(AppError::validation(field, message))
            } else {
                Ok(value)
            }
        })
        .transpose()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trim_and_limits_count_unicode_characters_and_preserve_missing_vs_empty() {
        assert_eq!(
            required_text(format!(" {} ", "ế".repeat(200)), "name", 200, "bad").unwrap(),
            "ế".repeat(200)
        );
        assert!(required_text("ế".repeat(201), "name", 200, "bad").is_err());
        assert!(required_text(" ".into(), "name", 200, "bad").is_err());
        assert_eq!(
            optional_text(None, "description", 2000, "bad").unwrap(),
            None
        );
        assert_eq!(
            optional_text(Some(" ".into()), "description", 2000, "bad").unwrap(),
            Some(String::new())
        );
    }
}
