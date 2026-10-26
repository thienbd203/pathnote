mod wire;
use super::dto::invalid;
use crate::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "wire::WireQuestion")]
pub struct Question {
    #[serde(flatten)]
    kind: Kind,
    prompt: String,
    options: Vec<String>,
    answers: Vec<String>,
    explanation: String,
    #[serde(rename = "caseSensitive")]
    case_sensitive: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Kind {
    Choice,
    Fill,
    Classify,
    Matching { items: Vec<String> },
    Ordering,
    Translation,
    Correction,
}
// AI import fingerprints include serialized questions. Preserve the original enum's
// field order as well as its JSON shape, including `items` last for matching.
impl Serialize for Question {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut value = serializer.serialize_struct(
            "Question",
            if matches!(self.kind, Kind::Matching { .. }) {
                7
            } else {
                6
            },
        )?;
        value.serialize_field("type", self.kind_name())?;
        value.serialize_field("prompt", &self.prompt)?;
        value.serialize_field("options", &self.options)?;
        value.serialize_field("answers", &self.answers)?;
        value.serialize_field("explanation", &self.explanation)?;
        value.serialize_field("caseSensitive", &self.case_sensitive)?;
        if let Kind::Matching { items } = &self.kind {
            value.serialize_field("items", items)?;
        }
        value.end()
    }
}
impl Question {
    fn kind_name(&self) -> &'static str {
        match &self.kind {
            Kind::Choice => "choice",
            Kind::Fill => "fill",
            Kind::Classify => "classify",
            Kind::Matching { .. } => "matching",
            Kind::Ordering => "ordering",
            Kind::Translation => "translation",
            Kind::Correction => "correction",
        }
    }

    pub fn validate(&mut self) -> Result<(), AppError> {
        self.prompt = self.prompt.trim().into();
        self.explanation = self.explanation.trim().into();
        let options = &mut self.options;
        let answers = &mut self.answers;
        for s in options.iter_mut().chain(answers.iter_mut()) {
            *s = s.trim().into();
        }
        if !text(&self.prompt, 2000)
            || self.explanation.chars().count() > 6000
            || answers.is_empty()
            || answers.len() > 10
            || !answers.iter().all(|s| text(s, 1000))
            || options.len() > 20
            || !options.iter().all(|s| text(s, 200))
        {
            return Err(invalid());
        }
        let valid = match &mut self.kind {
            Kind::Choice | Kind::Classify => {
                (2..=6).contains(&options.len())
                    && unique(options)
                    && answers.len() == 1
                    && options.contains(&answers[0])
            }
            Kind::Fill | Kind::Translation | Kind::Correction => options.is_empty(),
            Kind::Matching { items } => {
                for item in items.iter_mut() {
                    *item = item.trim().into();
                }
                (2..=8).contains(&items.len())
                    && items.iter().all(|s| text(s, 200))
                    && unique(items)
                    && unique(options)
                    && unique(answers)
                    && options.len() == items.len()
                    && answers.len() == items.len()
                    && answers.iter().all(|s| options.contains(s))
            }
            Kind::Ordering => {
                (2..=20).contains(&options.len())
                    && answers.len() == 1
                    && tokens(&answers[0]) == sorted(options)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid())
        }
    }
    pub fn public(&self) -> super::dto::PublicQuestionResource {
        super::dto::PublicQuestionResource {
            kind: self.kind_name(),
            prompt: self.prompt.clone(),
            options: self.options.clone(),
            items: if let Kind::Matching { items } = &self.kind {
                Some(items.clone())
            } else {
                None
            },
        }
    }
    pub fn answers(&self) -> &[String] {
        &self.answers
    }
    pub fn explanation(&self) -> &str {
        &self.explanation
    }
    pub fn answer_limit(&self) -> usize {
        if matches!(self.kind, Kind::Matching { .. }) {
            4000
        } else {
            1000
        }
    }
    pub fn grade(&self, answer: &str) -> Result<bool, AppError> {
        match &self.kind {
            Kind::Choice | Kind::Classify => {
                if !self.options.iter().any(|o| o == answer) {
                    return Err(AppError::validation(
                        "answer",
                        "Hãy chọn một phương án trong đề.",
                    ));
                }
                return Ok(self.answers[0] == answer);
            }
            Kind::Matching { items } => {
                let pairs: Vec<String> = serde_json::from_str(answer)
                    .map_err(|_| AppError::validation("answer", "Hãy nối đủ các cặp."))?;
                if pairs.len() != items.len()
                    || !unique(&pairs)
                    || pairs.iter().any(|s| !self.options.contains(s))
                {
                    return Err(AppError::validation(
                        "answer",
                        "Mỗi mục phải nối với một phương án khác nhau trong đề.",
                    ));
                }
                return Ok(pairs == self.answers);
            }
            Kind::Ordering if tokens(answer) != sorted(&self.options) => {
                return Err(AppError::validation(
                    "answer",
                    "Câu phải dùng đúng toàn bộ các từ trong đề.",
                ));
            }
            _ => {}
        }
        let normalize = |s: &str| {
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            // Translation may omit its final full stop; correction still compares punctuation.
            let s = if matches!(self.kind, Kind::Translation) {
                s.strip_suffix('.').unwrap_or(&s).trim_end()
            } else {
                &s
            };
            if self.case_sensitive {
                s.to_owned()
            } else {
                s.to_lowercase()
            }
        };
        Ok(self
            .answers()
            .iter()
            .any(|s| normalize(s) == normalize(answer)))
    }
}
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max
}
fn unique(s: &[String]) -> bool {
    s.iter().collect::<HashSet<_>>().len() == s.len()
}
fn tokens(s: &str) -> Vec<String> {
    let mut words = s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    words.sort();
    words
}
fn sorted(s: &[String]) -> Vec<String> {
    let mut words = s.to_vec();
    words.sort();
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn fixtures() -> Vec<Value> {
        ["choice", "fill", "classify", "matching", "ordering", "translation", "correction"]
            .into_iter().map(|kind| {
                let (options, answers) = match kind {
                    "choice" | "classify" => (json!(["is", "are"]), json!(["is"])),
                    "matching" => (json!(["A", "B"]), json!(["A", "B"])),
                    "ordering" => (json!(["I", "am"]), json!(["I am"])),
                    _ => (json!([]), json!(["as"])),
                };
                let mut q = json!({"type":kind,"prompt":"Prompt","options":options,"answers":answers,"explanation":"Reason","caseSensitive":false});
                if kind == "matching" { q["items"] = json!(["a", "b"]); }
                q
            }).collect()
    }
    #[test]
    fn all_seven_question_types_roundtrip_without_changing_json_or_exposing_answers() {
        for value in fixtures() {
            let mut q: Question = serde_json::from_value(value.clone()).unwrap();
            q.validate().unwrap();
            assert_eq!(serde_json::to_value(&q).unwrap(), value);
            let public = serde_json::to_value(q.public()).unwrap();
            assert!(public.get("answers").is_none());
            assert!(public.get("explanation").is_none());
            assert_eq!(public["type"], value["type"]);
        }
    }
    #[test]
    fn serialization_preserves_legacy_bytes_used_by_pending_ai_import_fingerprints() {
        for value in fixtures() {
            let q: Question = serde_json::from_value(value.clone()).unwrap();
            let fields = [
                "type",
                "prompt",
                "options",
                "answers",
                "explanation",
                "caseSensitive",
                "items",
            ];
            let legacy = format!(
                "{{{}}}",
                fields
                    .iter()
                    .filter_map(|field| value
                        .get(*field)
                        .map(|value| format!("\"{field}\":{value}")))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            assert_eq!(serde_json::to_string(&q).unwrap(), legacy);
        }
    }
    #[test]
    fn strict_input_rejects_extra_duplicate_and_type_specific_fields() {
        for mut value in fixtures() {
            let original = value.clone();
            value["extra"] = json!(true);
            assert!(serde_json::from_value::<Question>(value).is_err());
            let mut value = original.clone();
            value["items"] = Value::Null;
            assert!(serde_json::from_value::<Question>(value).is_err());
            if original["type"] != "matching" {
                let mut value = original;
                value["items"] = json!([]);
                assert!(serde_json::from_value::<Question>(value).is_err());
            } else {
                let mut value = original;
                value.as_object_mut().unwrap().remove("items");
                assert!(serde_json::from_value::<Question>(value).is_err());
            }
        }
        let value = serde_json::to_string(&fixtures()[1]).unwrap();
        let duplicate = value.replacen('{', "{\"prompt\":\"duplicate\",", 1);
        assert!(serde_json::from_str::<Question>(&duplicate).is_err());
    }
}
