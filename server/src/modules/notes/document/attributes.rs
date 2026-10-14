#![allow(dead_code)] // Field types constrain accepted JSON even when values need no extra rule.
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextAttrs {
    pub text_align: Option<Align>,
    pub indent: Option<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HeadingAttrs {
    pub level: u8,
    pub text_align: Option<Align>,
    pub indent: Option<u8>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct OrderedAttrs {
    pub start: Option<u32>,
    #[serde(rename = "type")]
    pub numbering: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAttrs {
    pub checked: bool,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeAttrs {
    pub language: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CellAttrs {
    pub colspan: Option<u8>,
    pub rowspan: Option<u8>,
    pub colwidth: Option<Vec<u16>>,
    pub align: Option<Align>,
}
#[derive(Deserialize, Serialize)]
pub enum Locale {
    #[serde(rename = "en-US")]
    Us,
    #[serde(rename = "en-GB")]
    Gb,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyAttrs {
    pub locale: Locale,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryAttrs {
    pub id: String,
    pub word: String,
    pub ipa: String,
    pub topics: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartOfSpeech {
    Verb,
    Noun,
    Adjective,
    Adverb,
    Preposition,
    Pronoun,
    Conjunction,
    Interjection,
    Phrase,
    Other,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SenseAttrs {
    pub id: String,
    pub part_of_speech: PartOfSpeech,
    pub meaning: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExampleAttrs {
    pub id: String,
    pub text: String,
    pub highlights: Vec<String>,
    pub ipa: Option<String>,
    pub meaning: Option<String>,
    pub meaning_highlights: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PronunciationAttrs {
    pub word: String,
    pub ipa: String,
    pub locale: Locale,
    pub meaning: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImageAttrs {
    pub media_id: i64,
    pub alt: Option<String>,
    pub align: Option<Align>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub caption_visible: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FileAttrs {
    pub media_id: i64,
}
