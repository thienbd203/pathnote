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
