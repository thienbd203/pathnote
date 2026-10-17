pub use super::question::Question;
use crate::AppError;
use serde::{Deserialize, Serialize};
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max
}
pub fn invalid() -> AppError {
    AppError::validation("questions", "Đề bài hoặc cấu trúc câu hỏi không hợp lệ.")
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadQuery {
    pub ids: String,
}
impl ReadQuery {
    pub fn validate(&self) -> Result<Vec<i64>, AppError> {
        let invalid = || AppError::validation("ids", "Cần từ 1 đến 100 ID bài tập hợp lệ.");
        let mut ids = Vec::new();
        for part in self.ids.split(',') {
            if ids.len() == 100 || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid());
            }
            let id = part.parse::<i64>().map_err(|_| invalid())?;
            if id <= 0 {
                return Err(invalid());
            }
            ids.push(id);
        }
        // Count the raw input before deduplicating, so repeated IDs cannot bypass the limit.
        let mut seen = std::collections::HashSet::new();
        ids.retain(|id| seen.insert(*id));
        Ok(ids)
    }
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
