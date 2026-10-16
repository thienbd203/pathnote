use serde_json::{json, Value};
pub fn paragraph() -> Value {
    json!({"type":"paragraph"})
}
fn heading(text: &str) -> Value {
    json!({"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":text}]})
}
fn vocabulary(locale: &str) -> Value {
    json!({"type":"vocabulary","attrs":{"locale":locale},"content":[{
        "type":"vocabularyEntry","attrs":{"id":uuid::Uuid::new_v4().to_string(),"word":"","ipa":"","topics":[]},"content":[{
            "type":"vocabularySense","attrs":{"id":uuid::Uuid::new_v4().to_string(),"partOfSpeech":"noun","meaning":""},"content":[{"type":"vocabularyExample","attrs":{"id":uuid::Uuid::new_v4().to_string(),"text":"","highlights":[],"ipa":null,"meaning":null,"meaningHighlights":null}}]
        }]
    }]})
}
// Copy layout and section names, never answers, file references, imported AI IDs or vocabulary IDs.
pub fn structure(node: &Value) -> Option<Value> {
    let kind = node["type"].as_str()?;
    match kind {
        "heading" => {
            let content = node["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|child| match child["type"].as_str() {
                    Some("text") => Some(json!({"type":"text","text":child["text"]})),
                    Some("hardBreak") => Some(json!({"type":"hardBreak"})),
                    _ => None,
                })
                .collect::<Vec<_>>();
            Some(
                json!({"type":"heading","attrs":{"level":node["attrs"]["level"]},"content":content}),
            )
        }
        "paragraph" => Some(paragraph()),
        "vocabulary" => Some(vocabulary(
            node["attrs"]["locale"].as_str().unwrap_or("en-US"),
        )),
        "codeBlock" => Some(json!({"type":"codeBlock"})),
        "horizontalRule" => Some(json!({"type":"horizontalRule"})),
        "bulletList" | "orderedList" | "taskList" | "listItem" | "taskItem" | "blockquote"
        | "table" | "tableRow" | "tableCell" | "tableHeader" => {
            let mut content = node["content"]
                .as_array()?
                .iter()
                .filter_map(structure)
                .collect::<Vec<_>>();
            if content.is_empty() {
                content.push(paragraph());
            }
            let mut result = json!({"type":kind,"content":content});
            if kind == "taskItem" {
                result["attrs"] = json!({"checked":false});
            }
            if matches!(kind, "tableCell" | "tableHeader") {
                result["attrs"] = node.get("attrs").cloned().unwrap_or(json!({}));
            }
            Some(result)
        }
        _ => None,
    }
}

pub fn from_nodes(content: Vec<Value>) -> Value {
    json!({"type":"doc","content":content})
}
pub fn empty() -> Value {
    from_nodes(vec![paragraph()])
}
pub fn lesson_template() -> Value {
    from_nodes(vec![
        heading("Ghi chú"),
        paragraph(),
        heading("Từ vựng"),
        vocabulary("en-US"),
        paragraph(),
    ])
}
pub fn vocabulary_import(locale: &super::Locale, entry: Value) -> Value {
    let senses = entry["senses"].as_array().expect("Serialized IPA senses").iter().map(|sense| {
        let examples = sense["examples"].as_array().expect("Serialized IPA examples").iter().map(|example| json!({"type":"vocabularyExample","attrs":{"id":uuid::Uuid::new_v4(),"text":example["text"],"ipa":example["ipa"],"meaning":example["meaning"],"highlights":example["highlights"],"meaningHighlights":example["meaningHighlights"]}})).collect::<Vec<_>>();
        json!({"type":"vocabularySense","attrs":{"id":uuid::Uuid::new_v4(),"partOfSpeech":sense["partOfSpeech"],"meaning":sense["meaning"]},"content":examples})
    }).collect::<Vec<_>>();
    json!({"type":"doc","content":[{"type":"vocabulary","attrs":{"locale":locale},"content":[{"type":"vocabularyEntry","attrs":{"id":uuid::Uuid::new_v4(),"word":entry["word"],"ipa":entry["ipa"],"topics":entry["topics"]},"content":senses}]}]})
}
