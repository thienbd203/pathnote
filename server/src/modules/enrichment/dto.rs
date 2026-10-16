use crate::AppError;
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ipa {
    pub text: String,
    pub locale: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub texts: Vec<String>,
    pub locale: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub text: String,
}
#[derive(Serialize)]
pub struct IpaResource {
    pub ipa: String,
    pub locale: String,
    pub engine: &'static str,
}
#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum BatchItemResource {
    Success { text: String, ipa: String },
    Error { text: String, error: String },
}
#[derive(Serialize)]
pub struct BatchResource {
    pub locale: String,
    pub engine: &'static str,
    pub results: Vec<BatchItemResource>,
}
#[derive(Serialize)]
pub struct TranslationResource {
    pub meaning: String,
    pub source: &'static str,
    pub target: &'static str,
    pub engine: &'static str,
}
#[derive(Deserialize)]
pub struct UpstreamIpa {
    pub locale: String,
    pub ipa: String,
}
#[derive(Deserialize)]
pub struct UpstreamBatch {
    pub locale: String,
    pub results: Vec<UpstreamItem>,
}
#[derive(Deserialize)]
pub struct UpstreamItem {
    pub text: String,
    pub ipa: Option<String>,
    pub error: Option<String>,
}
#[derive(Deserialize)]
pub struct UpstreamTranslation {
    #[serde(rename = "translatedText")]
    pub translated_text: String,
}
#[derive(Serialize)]
pub struct TranslationRequest<'a> {
    pub q: &'a str,
    pub source: &'static str,
    pub target: &'static str,
    pub format: &'static str,
}
pub fn validate_text(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty()
        || value.chars().count() > 1000
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(AppError::validation(
            "text",
            "Nội dung cần từ 1 đến 1000 ký tự.",
        ));
    }
    Ok(())
}
pub fn validate_locale(value: &str) -> Result<(), AppError> {
    if !matches!(value, "en-US" | "en-GB") {
        return Err(AppError::validation("locale", "Chọn US hoặc UK."));
    }
    Ok(())
}
