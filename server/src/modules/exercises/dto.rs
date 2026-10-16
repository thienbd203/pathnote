pub use super::question::Question;
use crate::AppError;
use serde::{Deserialize, Serialize};
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max
}
pub fn invalid() -> AppError {
    AppError::validation("questions", "Đề bài hoặc cấu trúc câu hỏi không hợp lệ.")
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub title: String,
    pub questions: Vec<Question>,
    #[serde(default)]
    pub titles: Option<Vec<String>>,
    pub revision: Option<i32>,
}
impl Definition {
    pub fn validate(&mut self, bulk: bool) -> Result<(), AppError> {
        self.title = self.title.trim().into();
        if !text(&self.title, 200)
            || self.questions.is_empty()
            || self.questions.len() > if bulk { 100 } else { 1 }
        {
            return Err(invalid());
        }
        if let Some(titles) = &mut self.titles {
            for title in titles.iter_mut() {
                *title = title.trim().into();
            }
            if !bulk || titles.len() != self.questions.len() || !titles.iter().all(|s| text(s, 200))
            {
                return Err(invalid());
            }
        }
        for q in &mut self.questions {
            q.validate()?;
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    #[serde(default)]
    pub restart: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub position: usize,
    pub answer: String,
    #[serde(rename = "try")]
    pub attempt: u8,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ResultData {
    pub position: usize,
    pub answer: String,
    pub tries: u8,
    pub correct: bool,
    pub r#final: bool,
}

#[derive(Serialize)]
pub struct PublicQuestionResource {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub prompt: String,
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
}
#[derive(Serialize)]
pub struct AttemptResultResource {
    #[serde(flatten)]
    pub result: ResultData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}
#[derive(Serialize)]
pub struct AttemptResource {
    pub id: i64,
    pub revision: i32,
    pub position: usize,
    pub completed: bool,
    pub questions: Vec<PublicQuestionResource>,
    pub results: Vec<AttemptResultResource>,
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum CreateResource {
    One { id: i64 },
    Many { ids: Vec<i64> },
}
#[derive(Serialize)]
pub struct IdResource {
    pub id: i64,
}
#[derive(Serialize)]
pub struct DefinitionResource {
    pub title: String,
    pub questions: Vec<Question>,
    pub revision: i32,
}
#[derive(Serialize)]
pub struct ExerciseResource {
    pub id: i64,
    pub title: String,
    pub revision: i32,
    pub can_manage: bool,
    pub questions: Vec<PublicQuestionResource>,
    pub attempt: Option<AttemptResource>,
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum ShowResource {
    Definition(DefinitionResource),
    Exercise(ExerciseResource),
}
