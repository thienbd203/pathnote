//! Serde defines the wire types; traversal handles document-wide invariants.
mod attributes;
mod marks;
mod node;
mod validation;
use node::Node;
use serde::Deserialize;
pub use validation::validate;
pub struct MediaReference {
    pub id: i64,
    pub kind: &'static str,
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
