use crate::{
    modules::notes::document::{self, Locale, PartOfSpeech},
    AppError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
        let mut senses = Vec::new();
        for sense in &mut self.senses {
            sense.meaning = sense.meaning.trim().into();
            let mut examples = Vec::new();
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
                examples.push(json!({"type":"vocabularyExample","attrs":{"id":uuid::Uuid::new_v4(),"text":e.text,"ipa":e.ipa,"meaning":e.meaning,"highlights":e.highlights,"meaningHighlights":e.meaning_highlights}}));
            }
            senses.push(json!({"type":"vocabularySense","attrs":{"id":uuid::Uuid::new_v4(),"partOfSpeech":sense.part_of_speech,"meaning":sense.meaning},"content":examples}));
        }
        let entry = json!({"type":"vocabularyEntry","attrs":{"id":uuid::Uuid::new_v4(),"word":self.word,"ipa":self.ipa,"topics":self.topics},"content":senses});
        document::validate(
            &json!({"type":"doc","content":[{"type":"vocabulary","attrs":{"locale":locale},"content":[entry]}]}),
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
