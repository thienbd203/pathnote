use super::dto::*;
use crate::{
    entities::{sessions, vocabulary_cards as cards, vocabulary_reviews as reviews},
    modules::notes::service as notes,
    AppError,
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub fn key(card: &cards::Model) -> String {
    key_fields(
        &card.word,
        &card.locale,
        &card.part_of_speech,
        &card.meaning,
    )
}
pub(super) fn key_fields(word: &str, locale: &str, part_of_speech: &str, meaning: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                super::super::sync::normalize(word),
                locale,
                part_of_speech,
                super::super::sync::normalize(meaning),
            ))
            .expect("String serialization")
        )
    )
}

pub struct Review {
    pub model: reviews::Model,
    pub mode: Mode,
    pub questions: Vec<Question>,
    pub answers: Vec<Answer>,
}
impl Review {
    pub fn parse(model: reviews::Model) -> Result<Self, AppError> {
        let questions = serde_json::from_value(model.questions.clone())
            .map_err(|_| AppError::Internal("Invalid review questions".into()))?;
        let answers = serde_json::from_value(model.answers.clone())
            .map_err(|_| AppError::Internal("Invalid review answers".into()))?;
        let mode = match model.mode.as_str() {
            "typing" => Mode::Typing,
            "choice" => Mode::Choice,
            _ => return Err(AppError::Internal("Invalid study mode".into())),
        };
        Ok(Self {
            model,
            mode,
            questions,
            answers,
        })
    }
    pub async fn authorize<C: ConnectionTrait>(&self, db: &C) -> Result<(), AppError> {
        let ids: BTreeSet<_> = self
            .questions
            .iter()
            .flat_map(|q| q.source_sessions.iter().copied())
            .collect();
        let count = notes::accessible(self.model.user_id, false)
            .filter(sessions::Column::Id.is_in(ids.iter().copied()))
            .count(db)
            .await?;
        if count != ids.len() as u64 {
            return Err(AppError::NotFound("Không tìm thấy lượt ôn.".into()));
        }
        Ok(())
    }
    pub async fn current<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> Result<Option<cards::Model>, AppError> {
        let Some(q) = self.questions.get(self.answers.len()) else {
            return Ok(None);
        };
        let card = cards::Entity::find_by_id(q.card_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy từ vựng.".into()))?;
        if !card.active || key(&card) != q.study_key {
            return Err(AppError::Conflict(
                "Từ hoặc nghĩa đã thay đổi. Hãy tạo lượt ôn mới.".into(),
            ));
        }
        Ok(Some(card))
    }
    pub fn state(&self) -> ReviewResource {
        let question = self
            .questions
            .get(self.answers.len())
            .map(|q| QuestionResource {
                position: self.answers.len(),
                meaning: q.card.meaning.clone(),
                part_of_speech: q.card.part_of_speech.clone(),
                options: q.options.clone(),
                attempt: u8::from(q.first_attempt.is_some()),
                retry: q.first_attempt.as_ref().map(|answer| Retry {
                    answer: answer.clone(),
                    pattern: if self.mode == Mode::Typing {
                        pattern(&q.card.word)
                    } else {
                        Vec::new()
                    },
                }),
                ipa: (self.mode == Mode::Typing).then(|| q.card.ipa.clone()),
                locale: (self.mode == Mode::Typing).then(|| q.card.locale.clone()),
                letters: (self.mode == Mode::Typing).then(|| q.letters.clone()),
            });
        ReviewResource {
            id: self.model.id,
            mode: self.mode,
            filters: self.model.filters.clone(),
            total: self.questions.len(),
            answered: self.answers.len(),
            correct: self.answers.iter().filter(|a| a.correct).count(),
            complete: question.is_none(),
            mistakes: if question.is_none() {
                self.answers
                    .iter()
                    .filter(|a| !a.correct || a.assisted)
                    .map(|a| a.card.clone())
                    .collect()
            } else {
                Vec::new()
            },
            question,
        }
    }
}
fn pattern(word: &str) -> Vec<Option<String>> {
    let total = word.chars().filter(|c| c.is_alphanumeric()).count();
    let mut index = 0;
    word.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                let hide = total == 1 || index % 2 == 1;
                index += 1;
                if hide {
                    return None;
                }
            }
            Some(c.to_string())
        })
        .collect()
}
pub async fn load<C: ConnectionTrait>(db: &C, user: i64, id: i64) -> Result<Review, AppError> {
    let model = reviews::Entity::find_by_id(id)
        .filter(reviews::Column::UserId.eq(user))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy lượt ôn.".into()))?;
    let review = Review::parse(model)?;
    review.authorize(db).await?;
    review.current(db).await?;
    Ok(review)
}
