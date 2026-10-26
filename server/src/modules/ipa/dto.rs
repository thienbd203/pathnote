use crate::{
    modules::notes::document::{self, Locale, PartOfSpeech},
    AppError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub word: String,
    #[serde(default)]
    pub ipa: String,
    #[serde(default, rename = "ipaUK", skip_serializing_if = "Option::is_none")]
    pub ipa_uk: Option<String>,
    #[serde(default, rename = "ipaUS", skip_serializing_if = "Option::is_none")]
    pub ipa_us: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    pub senses: Vec<Sense>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Sense {
    pub part_of_speech: PartOfSpeech,
    #[serde(default)]
    pub meaning: String,
    pub examples: Vec<Example>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Example {
    pub text: String,
    #[serde(default)]
    pub ipa: String,
    #[serde(default, rename = "ipaUK", skip_serializing_if = "Option::is_none")]
    pub ipa_uk: Option<String>,
    #[serde(default, rename = "ipaUS", skip_serializing_if = "Option::is_none")]
    pub ipa_us: Option<String>,
    #[serde(default)]
    pub meaning: String,
    #[serde(default)]
    pub highlights: Vec<String>,
    #[serde(default)]
    pub meaning_highlights: Vec<String>,
}
fn trim_accents(
    ipa: &mut String,
    uk: &mut Option<String>,
    us: &mut Option<String>,
    locale: &Locale,
) {
    for value in [&mut *uk, &mut *us].into_iter().flatten() {
        *value = value.trim().into();
    }
    *ipa = match locale {
        Locale::Gb => uk.as_deref(),
        Locale::Us => us.as_deref(),
    }
    .filter(|v| !v.is_empty())
    .unwrap_or(ipa)
    .trim()
    .into();
}
impl Entry {
    pub fn validate(&mut self, locale: &Locale) -> Result<(), AppError> {
        self.word = self.word.trim().into();
        trim_accents(&mut self.ipa, &mut self.ipa_uk, &mut self.ipa_us, locale);
        for topic in &mut self.topics {
            *topic = topic.trim().into();
        }
        if self.word.is_empty() {
            return Err(invalid());
        }
        for sense in &mut self.senses {
            sense.meaning = sense.meaning.trim().into();
            for e in &mut sense.examples {
                e.text = e.text.trim().into();
                trim_accents(&mut e.ipa, &mut e.ipa_uk, &mut e.ipa_us, locale);
                e.meaning = e.meaning.trim().into();
                for h in e
                    .highlights
                    .iter_mut()
                    .chain(e.meaning_highlights.iter_mut())
                {
                    *h = h.trim().into();
                }
                if e.highlights.iter().any(|h| !e.text.contains(h))
                    || e.meaning_highlights.iter().any(|h| !e.meaning.contains(h))
                {
                    return Err(invalid());
                }
            }
        }
        document::validate(
            &document::builders::vocabulary_import(locale, value(self)?),
            crate::modules::notes::schema::NOTE_SCHEMA_VERSION,
        )?;
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Words {
    pub version: u8,
    pub locale: Locale,
    pub entries: Vec<Entry>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub left: Entry,
    pub right: Entry,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pairs {
    pub version: u8,
    pub kind: String,
    pub locale: Locale,
    pub sounds: [String; 2],
    pub pairs: Vec<Pair>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub locale: Locale,
    #[serde(default = "first")]
    pub page: u64,
}
fn first() -> u64 {
    1
}
pub fn invalid() -> AppError {
    AppError::validation("entries", "Từ hoặc cặp âm IPA không hợp lệ.")
}
pub fn locale(l: &Locale) -> &'static str {
    match l {
        Locale::Us => "en-US",
        Locale::Gb => "en-GB",
    }
}
pub fn key(word: &str) -> String {
    word.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
pub fn value<T: Serialize>(data: &T) -> Result<Value, AppError> {
    serde_json::to_value(data).map_err(|e| AppError::Internal(e.to_string()))
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum LibraryData {
    Words {
        version: u8,
        locale: String,
        entries: Vec<Entry>,
    },
    Pairs {
        version: u8,
        locale: String,
        sets: Vec<PairSet>,
    },
}
#[derive(Serialize)]
pub struct PairSet {
    pub sounds: [String; 2],
    pub pairs: Vec<PairContent>,
}
#[derive(Serialize)]
pub struct PairContent {
    pub left: Entry,
    pub right: Entry,
}
#[derive(Serialize)]
pub struct ListResource {
    pub data: LibraryData,
    pub meta: crate::pagination::PageMeta,
}
#[derive(Serialize)]
pub struct ImportResource {
    pub added: usize,
    pub skipped: Vec<String>,
}

#[derive(Serialize)]
pub struct StoredPair<'a> {
    pub sounds: &'a [String; 2],
    pub left: &'a Entry,
    pub right: &'a Entry,
}

#[derive(Serialize)]
pub struct Duplicates {
    pub duplicates: Vec<String>,
}

#[cfg(test)]
mod dual_accent_tests {
    use super::*;
    #[test]
    fn shared_vocabulary_json_keeps_both_accents_and_validates_the_unused_one() {
        let input = serde_json::json!({"word":"history","ipa":"/old/","ipaUK":" /UK/ ","ipaUS":" /US/ ","topics":[],"senses":[{"partOfSpeech":"noun","meaning":"lịch sử","examples":[{"text":"I study history.","ipaUK":"/UK sentence/","ipaUS":"/US sentence/"}]}]});
        let mut entry: Entry = serde_json::from_value(input.clone()).unwrap();
        entry.validate(&Locale::Us).unwrap();
        assert_eq!(entry.ipa, "/US/");
        assert_eq!(value(&entry).unwrap()["ipaUK"], "/UK/");
        let mut invalid = input;
        invalid["ipaUK"] = serde_json::json!("x".repeat(6001));
        let mut invalid: Entry = serde_json::from_value(invalid).unwrap();
        assert!(invalid.validate(&Locale::Us).is_err());
    }
}
