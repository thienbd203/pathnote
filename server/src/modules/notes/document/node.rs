use super::{attributes::*, marks::Mark};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Node {
    #[serde(rename = "image")]
    Image {
        attrs: ImageAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "file")]
    File {
        attrs: FileAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        marks: Vec<Mark>,
    },
    #[serde(rename = "hardBreak")]
    HardBreak {
        #[serde(default)]
        marks: Vec<Mark>,
    },
    #[serde(rename = "paragraph")]
    Paragraph {
        #[serde(default)]
        attrs: TextAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "heading")]
    Heading {
        attrs: HeadingAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "bulletList")]
    BulletList {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "orderedList")]
    OrderedList {
        #[serde(default)]
        attrs: OrderedAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "listItem")]
    ListItem {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "taskList")]
    TaskList {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "taskItem")]
    TaskItem {
        #[serde(rename = "attrs")]
        _attrs: TaskAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "blockquote")]
    Blockquote {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "codeBlock")]
    CodeBlock {
        #[serde(default)]
        attrs: CodeAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "table")]
    Table {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "tableRow")]
    TableRow {
        #[serde(default)]
        #[serde(rename = "attrs")]
        _attrs: Empty,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "tableCell")]
    TableCell {
        #[serde(default)]
        attrs: CellAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "tableHeader")]
    TableHeader {
        #[serde(default)]
        attrs: CellAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "vocabulary")]
    Vocabulary {
        #[serde(rename = "attrs")]
        _attrs: VocabularyAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "vocabularyEntry")]
    VocabularyEntry {
        attrs: EntryAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "vocabularySense")]
    VocabularySense {
        attrs: SenseAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "vocabularyExample")]
    VocabularyExample {
        attrs: ExampleAttrs,
        #[serde(default)]
        content: Vec<Node>,
    },
    #[serde(rename = "horizontalRule")]
    HorizontalRule {},
    #[serde(rename = "pronunciation")]
    Pronunciation { attrs: PronunciationAttrs },
}
