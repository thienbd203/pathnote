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
    #[serde(default)]
    pub meaning: String,
    #[serde(default)]
    pub highlights: Vec<String>,
    #[serde(default)]
    pub meaning_highlights: Vec<String>,
}
impl Entry {
    pub fn validate(&mut self, locale: &Locale) -> Result<(), AppError> {
        self.word = self.word.trim().into();
        self.ipa = self.ipa.trim().into();
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
                e.ipa = e.ipa.trim().into();
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
            3,
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
