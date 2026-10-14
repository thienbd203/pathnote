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
