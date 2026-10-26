use super::{Kind, Question};
use serde::{Deserialize, Deserializer};

// Serde explicitly does not support flatten + deny_unknown_fields together
// (https://serde.rs/attr-flatten.html). Keep a strict typed input adapter so the
// shared internal representation does not silently accept unknown/type-specific fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireQuestion {
    #[serde(rename = "type")]
    kind: Type,
    prompt: String,
    options: Vec<String>,
    answers: Vec<String>,
    explanation: String,
    #[serde(rename = "caseSensitive")]
    case_sensitive: bool,
    #[serde(default, deserialize_with = "present_items")]
    items: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Type {
    Choice,
    Fill,
    Classify,
    Matching,
    Ordering,
    Translation,
    Correction,
}
fn present_items<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    // Missing is allowed for non-matching questions; explicit null is never valid.
    Vec::<String>::deserialize(d).map(Some)
}
impl TryFrom<WireQuestion> for Question {
    type Error = &'static str;
    fn try_from(wire: WireQuestion) -> Result<Self, Self::Error> {
        if wire.items.is_some() && !matches!(wire.kind, Type::Matching) {
            return Err("items is only supported for matching questions");
        }
        let kind = match wire.kind {
            Type::Choice => Kind::Choice,
            Type::Fill => Kind::Fill,
            Type::Classify => Kind::Classify,
            Type::Matching => Kind::Matching {
                items: wire.items.ok_or("matching questions require items")?,
            },
            Type::Ordering => Kind::Ordering,
            Type::Translation => Kind::Translation,
            Type::Correction => Kind::Correction,
        };
        Ok(Self {
            kind,
            prompt: wire.prompt,
            options: wire.options,
            answers: wire.answers,
            explanation: wire.explanation,
            case_sensitive: wire.case_sensitive,
        })
    }
}
