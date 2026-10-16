use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Typing,
    Choice,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Typing => "typing",
            Self::Choice => "choice",
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub mode: Mode,
    pub limit: Option<usize>,
    pub session_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<i64>,
    pub topic: Option<String>,
    pub q: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerInput {
    pub position: usize,
    pub answer: String,
    #[serde(default)]
    pub attempt: u8,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Card {
    pub word: String,
    pub meaning: String,
    pub ipa: String,
    pub locale: String,
    pub part_of_speech: String,
    pub topics: Value,
    pub examples: Value,
    pub source: Value,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Question {
    pub card_id: i64,
    pub study_key: String,
    pub source_sessions: Vec<i64>,
    pub card: Card,
    pub options: Vec<String>,
    pub letters: Vec<String>,
    pub first_attempt: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Answer {
    pub position: usize,
    pub answer: String,
    pub correct: bool,
    pub assisted: bool,
    pub attempt: u8,
    pub card: Card,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub position: usize,
}
#[derive(Serialize)]
pub struct Pronunciation {
    pub text: String,
    pub locale: String,
}
#[derive(Serialize)]
pub struct Retry {
    pub answer: String,
    pub pattern: Vec<Option<String>>,
}
#[derive(Serialize)]
pub struct QuestionResource {
    pub position: usize,
    pub meaning: String,
    pub part_of_speech: String,
    pub options: Vec<String>,
    pub attempt: u8,
    pub retry: Option<Retry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipa: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letters: Option<Vec<String>>,
}
#[derive(Serialize)]
pub struct ReviewResource {
    pub id: i64,
    pub mode: Mode,
    pub filters: Value,
    pub total: usize,
    pub answered: usize,
    pub correct: usize,
    pub complete: bool,
    pub question: Option<QuestionResource>,
    pub mistakes: Vec<Card>,
}
#[derive(Serialize)]
pub struct AnswerResource {
    pub result: Option<Answer>,
    pub review: ReviewResource,
}

#[derive(Serialize)]
pub struct Source {
    pub id: i64,
    pub title: String,
    pub shared: bool,
}
