use super::{node::Node, Document};
use crate::AppError;
use serde_json::Value;
use std::collections::BTreeSet;

pub struct ExtractedSense {
    pub entry_id: String,
    pub sense_id: String,
    pub word: String,
    pub locale: String,
    pub ipa: String,
    pub part_of_speech: String,
    pub meaning: String,
    pub topics: Value,
    pub examples: Value,
}
fn label<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .expect("String enum")
        .as_str()
        .expect("String enum")
        .to_owned()
}
/// Input has already passed the document-wide shape/ID/media validation on save.
pub fn extract(value: &Value) -> Result<Vec<ExtractedSense>, AppError> {
    let document: Document = serde_json::from_value(value.clone())
        .map_err(|_| AppError::validation("content", "Ghi chép không hợp lệ."))?;
    let mut cards = Vec::new();
    for node in &document.content {
        collect(node, &mut cards);
    }
    Ok(cards)
}
fn collect(node: &Node, cards: &mut Vec<ExtractedSense>) {
    if let Node::Vocabulary {
        attrs: table,
        content,
    } = node
    {
        for entry in content {
            let Node::VocabularyEntry {
                attrs,
                content: senses,
            } = entry
            else {
                continue;
            };
            let word = attrs.word.trim();
            if word.is_empty() {
                continue;
            }
            let topics: BTreeSet<_> = attrs
                .topics
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            for sense in senses {
                let Node::VocabularySense {
                    attrs: sense,
                    content: examples,
                } = sense
                else {
                    continue;
                };
                let examples: Vec<_> = examples.iter().filter_map(|e| match e {
                    Node::VocabularyExample { attrs, .. } => Some(serde_json::json!({"id":attrs.id,"text":attrs.text.trim(),"ipa":attrs.ipa.as_deref().unwrap_or("").trim(),"meaning":attrs.meaning.as_deref().unwrap_or("").trim(),"highlights":attrs.highlights,"meaningHighlights":attrs.meaning_highlights.as_deref().unwrap_or_default()})),
                    _ => None,
                }).collect();
                cards.push(ExtractedSense {
                    entry_id: attrs.id.clone(),
                    sense_id: sense.id.clone(),
                    word: word.into(),
                    locale: label(&table.locale),
                    ipa: attrs.ipa.trim().into(),
                    part_of_speech: label(&sense.part_of_speech),
                    meaning: sense.meaning.trim().into(),
                    topics: serde_json::json!(topics),
                    examples: serde_json::json!(examples),
                });
            }
        }
    } else {
        for child in node.children() {
            collect(child, cards);
        }
    }
}
