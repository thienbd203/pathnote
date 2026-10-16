use super::{
    dto::*,
    state::{key, Review},
};
use crate::{
    entities::{places, sessions, vocabulary_progress as progress, vocabulary_reviews as reviews},
    modules::places::service as place_service,
    AppError,
};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait,
};

use std::collections::BTreeSet;
fn conflict() -> AppError {
    AppError::Conflict("Câu hỏi hoặc lần thử đã thay đổi. Hãy tải lại lượt ôn.".into())
}
pub async fn answer(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    input: AnswerInput,
) -> Result<AnswerResource, AppError> {
    if input.position >= 50
        || input.attempt > 1
        || input.answer.trim().is_empty()
        || input.answer.chars().count() > 1000
    {
        return Err(AppError::validation("answer", "Đáp án không hợp lệ."));
    }
    let txn = db.begin().await?;
    let model = reviews::Entity::find_by_id(id)
        .filter(reviews::Column::UserId.eq(user))
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy lượt ôn.".into()))?;
    let mut review = Review::parse(model)?;
    // Note saves and share revocation take the same parent locks. Order prevents cross-review deadlocks.
    let sessions: BTreeSet<_> = review
        .questions
        .iter()
        .flat_map(|q| q.source_sessions.iter().copied())
        .collect();
    let parents: BTreeSet<_> = sessions::Entity::find()
        .filter(sessions::Column::Id.is_in(sessions))
        .all(&txn)
        .await?
        .into_iter()
        .map(|s| s.place_id)
        .collect();
    for parent in parents {
        let place = places::Entity::find_by_id(parent)
            .one(&txn)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy bài học.".into()))?;
        place_service::lock_for_write(&txn, place.user_id, parent).await?;
    }
    review.authorize(&txn).await?;
    if let Some(previous) = review.answers.get(input.position) {
        if previous.attempt != input.attempt
            || super::super::sync::normalize(&previous.answer)
                != super::super::sync::normalize(&input.answer)
        {
            return Err(conflict());
        }
        return Ok(AnswerResource {
            result: Some(previous.clone()),
            review: review.state(),
        });
    }
    if input.position != review.answers.len() {
        return Err(conflict());
    }
    let current = review.current(&txn).await?.ok_or_else(conflict)?;
    let question = review
        .questions
        .get_mut(input.position)
        .ok_or_else(conflict)?;
    question.card.ipa = current.ipa.clone();
    if review.mode == Mode::Choice && !question.options.contains(&input.answer) {
        return Err(AppError::validation(
            "answer",
            "Hãy chọn một phương án trong danh sách.",
        ));
    }
    let assisted = question.first_attempt.is_some();
    if assisted && input.attempt == 0 {
        if question
            .first_attempt
            .as_ref()
            .map(|a| super::super::sync::normalize(a))
            != Some(super::super::sync::normalize(&input.answer))
        {
            return Err(conflict());
        }
        return Ok(AnswerResource {
            result: None,
            review: review.state(),
        });
    }
    if input.attempt != u8::from(assisted) {
        return Err(conflict());
    }
    let correct = super::super::sync::normalize(&input.answer)
        == super::super::sync::normalize(&question.card.word);
    let result = if !correct && !assisted {
        question.first_attempt = Some(input.answer.trim().into());
        None
    } else {
        let old = progress::Entity::find_by_id((user, current.id))
            .one(&txn)
            .await?
            .filter(|p| p.study_key == key(&current));
        let correct_count = old
            .as_ref()
            .map_or(0, |p| p.correct)
            .saturating_add(i32::from(correct));
        let incorrect = old
            .as_ref()
            .map_or(0, |p| p.incorrect)
            .saturating_add(i32::from(!correct || assisted));
        let streak = if correct && !assisted {
            old.as_ref().map_or(0, |p| p.streak).saturating_add(1)
        } else {
            0
        };
        let now = chrono::Utc::now().fixed_offset();
        let due = now
            + if streak > 0 {
                chrono::Duration::days([1, 3, 7, 14, 30][((streak - 1) as usize).min(4)])
            } else {
                chrono::Duration::minutes(10)
            };
        progress::Entity::insert(progress::ActiveModel {
            user_id: Set(user),
            card_id: Set(current.id),
            study_key: Set(key(&current)),
            correct: Set(correct_count),
            incorrect: Set(incorrect),
            streak: Set(streak),
            due_at: Set(due),
            last_reviewed_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([progress::Column::UserId, progress::Column::CardId])
                .update_columns([
                    progress::Column::StudyKey,
                    progress::Column::Correct,
                    progress::Column::Incorrect,
                    progress::Column::Streak,
                    progress::Column::DueAt,
                    progress::Column::LastReviewedAt,
                ])
                .to_owned(),
        )
        .exec(&txn)
        .await?;
        let result = Answer {
            position: input.position,
            answer: input.answer,
            correct,
            assisted,
            attempt: input.attempt,
            card: question.card.clone(),
        };
        review.answers.push(result.clone());
        Some(result)
    };
    let mut model: reviews::ActiveModel = review.model.clone().into();
    model.questions =
        Set(serde_json::to_value(&review.questions)
            .map_err(|e| AppError::Internal(e.to_string()))?);
    model.answers =
        Set(serde_json::to_value(&review.answers).map_err(|e| AppError::Internal(e.to_string()))?);
    model.updated_at = Set(chrono::Utc::now().fixed_offset());
    review.model = model.update(&txn).await?;
    let result = AnswerResource {
        result,
        review: review.state(),
    };
    txn.commit().await?;
    Ok(result)
}
