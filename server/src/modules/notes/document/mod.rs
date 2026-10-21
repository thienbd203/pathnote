pub(in crate::modules) mod builders;
// Serde defines the wire types; traversal handles document-wide invariants.
mod attributes;
mod marks;
mod node;
mod validation;
use node::Node;
use serde::Deserialize;
pub use validation::validate;
pub enum Reference {
    AIStudy(uuid::Uuid),
    Media { id: i64, kind: &'static str },
    Exercise(i64),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    #[serde(rename = "type")]
    pub kind: Root,
    pub content: Vec<Node>,
}
#[derive(Deserialize)]
pub enum Root {
    #[serde(rename = "doc")]
    Doc,
}

mod vocabulary;
pub(in crate::modules) use vocabulary::{extract, ExtractedSense};

pub(in crate::modules) use attributes::{Locale, PartOfSpeech};

#[cfg(test)]
mod dual_accent_tests {
    use super::validate;
    use serde_json::json;

    #[test]
    fn vocabulary_accepts_independent_accents_but_checks_each_field() {
        let mut doc = json!({"type":"doc","content":[{"type":"vocabulary","attrs":{"locale":"en-US"},"content":[{"type":"vocabularyEntry","attrs":{"id":"00000000-0000-0000-0000-000000000001","word":"history","ipa":"/US/","ipaUK":"/UK/","ipaUS":"/US/","topics":[]},"content":[{"type":"vocabularySense","attrs":{"id":"00000000-0000-0000-0000-000000000002","partOfSpeech":"noun","meaning":"lịch sử"},"content":[{"type":"vocabularyExample","attrs":{"id":"00000000-0000-0000-0000-000000000003","text":"I study history.","highlights":[],"ipaUK":"/UK sentence/","ipaUS":"/US sentence/"}}]}]}]}]});
        assert!(validate(&doc, 3).is_ok());
        for field in ["ipaUK", "ipaUS"] {
            let path = &mut doc["content"][0]["content"][0]["attrs"];
            path[field] = json!("x".repeat(6001));
            assert!(validate(&doc, 3).is_err());
            doc["content"][0]["content"][0]["attrs"][field] = json!("/valid/");
            doc["content"][0]["content"][0]["content"][0]["content"][0]["attrs"][field] =
                json!(false);
            assert!(validate(&doc, 3).is_err());
            doc["content"][0]["content"][0]["content"][0]["content"][0]["attrs"][field] =
                json!("/valid/");
        }
        assert!(validate(&doc, 3).is_ok());
    }
}
