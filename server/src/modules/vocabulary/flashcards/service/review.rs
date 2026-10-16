use super::super::{dto, personal, queue, scheduler};
use crate::{
    entities::{
        flashcard_attempts as attempts, flashcard_memory as memory, users,
        vocabulary_cards as cards,
    },
    modules::{notes::service as notes, places::service as places},
    AppError,
};
use sea_orm::{
    sea_query::{LockType, OnConflict},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Set,
};

use uuid::Uuid;
async fn ticket<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: Uuid,
    lock: bool,
) -> Result<attempts::Model, AppError> {
    let mut query = attempts::Entity::find_by_id(id).filter(attempts::Column::UserId.eq(user));
    if lock {
        query = query.lock(LockType::Update);
    }
    query
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy thẻ ôn.".into()))
}
async fn card<C: ConnectionTrait>(
    db: &C,
    user: i64,
    t: &attempts::Model,
) -> Result<cards::Model, AppError> {
    let card = crate::modules::vocabulary::library::scope(user, None)
        .filter(cards::Column::Id.eq(t.card_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy từ vựng.".into()))?;
    if queue::content_key(&card) != t.content_key {
        return Err(AppError::Conflict(
            "Từ hoặc nghĩa đã thay đổi. Hãy lấy thẻ mới.".into(),
        ));
    }
    Ok(card)
}
fn fresh(t: &attempts::Model, m: Option<&memory::Model>) -> Result<(), AppError> {
    if chrono::Utc::now().fixed_offset() - t.created_at > chrono::Duration::hours(24)
        || t.version != m.map_or(0, |m| m.reviews)
        || m.is_some_and(|m| m.card_id != t.memory_card_id)
    {
        return Err(AppError::Conflict(
            "Thẻ này đã được ôn hoặc hết hạn. Hãy lấy thẻ mới.".into(),
        ));
    }
    Ok(())
}
pub async fn reveal(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    id: Uuid,
) -> Result<dto::Reveal, AppError> {
    let t = ticket(db, user, id, false).await?;
    let c = card(db, user, &t).await?;
    let (_, place) = notes::access(db, user, c.session_id, false).await?;
    let txn = places::write_tx(db, place.user_id, place.id).await?;
    let t = ticket(&txn, user, id, true).await?;
    let c = card(&txn, user, &t).await?;
    let profile = personal::for_ticket(&t)?;
    let m = queue::memory(&txn, user, &c, &t.direction).await?;
    let m = profile.memory(&txn, user, m).await?;
    fresh(&t, m.as_ref())?;
    let options = scheduler::outcomes(
        &profile.scheduler()?,
        m.as_ref(),
        t.retention,
        chrono::Utc::now().fixed_offset(),
    )?;
    let meaning = queue::meanings(&txn, user, &c).await?;
    let mut model: attempts::ActiveModel = t.into();
    model.revealed = Set(true);
    model.snapshot = Set(Some(
        serde_json::to_value(dto::Snapshot {
            word: c.word.clone(),
            meaning: meaning.clone(),
            ipa: c.ipa.clone(),
            locale: c.locale.clone(),
            part_of_speech: c.part_of_speech.clone(),
            topics: c.topics.clone(),
            examples: c.examples.clone(),
        })
        .map_err(|e| AppError::Internal(e.to_string()))?,
    ));
    model.update(&txn).await?;
    txn.commit().await?;
    Ok(dto::Reveal {
        back: dto::Back {
            word: c.word,
            meaning,
            ipa: c.ipa,
            locale: c.locale,
            part_of_speech: c.part_of_speech,
            examples: c.examples,
        },
        options,
    })
}
pub async fn grade(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    id: Uuid,
    input: dto::Grade,
) -> Result<dto::GradeResult, AppError> {
    if !(1..=4).contains(&input.rating) {
        return Err(AppError::validation(
            "rating",
            "Chọn một trong bốn mức ghi nhớ.",
        ));
    }
    let t = ticket(db, user, id, false).await?;
    let c = card(db, user, &t).await?;
    let (_, place) = notes::access(db, user, c.session_id, false).await?;
    let txn = places::write_tx(db, place.user_id, place.id).await?;
    // Serializes updates to this user's memory, including two independently opened cards.
    users::Entity::find_by_id(user)
        .lock(LockType::Update)
        .one(&txn)
        .await?;
    let t = ticket(&txn, user, id, true).await?;
    let c = card(&txn, user, &t).await?;
    if let Some(rating) = t.rating {
        if rating != i32::from(input.rating) {
            return Err(AppError::Conflict("Thẻ đã được đánh giá.".into()));
        }
        return Ok(dto::GradeResult {
            rating,
            due_at: t.due_at,
        });
    }
    if let Some(run_id) = t.run_id {
        let run = crate::entities::flashcard_runs::Entity::find_by_id(run_id)
            .filter(crate::entities::flashcard_runs::Column::UserId.eq(user))
            .lock(LockType::Update)
            .one(&txn)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy lượt ôn.".into()))?;
        if run.finished_at.is_some() {
            return Err(AppError::Conflict("Lượt ôn đã kết thúc.".into()));
        }
    }
    if !t.revealed {
        return Err(AppError::Conflict(
            "Hãy xem đáp án trước khi đánh giá.".into(),
        ));
    }
    let profile = personal::for_ticket(&t)?;
    let m = queue::memory(&txn, user, &c, &t.direction).await?;
    let m = profile.memory(&txn, user, m).await?;
    fresh(&t, m.as_ref())?;
    let now = chrono::Utc::now().fixed_offset();
    let result = scheduler::outcomes(&profile.scheduler()?, m.as_ref(), t.retention, now)?
        .remove(input.rating as usize - 1);
    memory::Entity::insert(memory::ActiveModel {
        user_id: Set(user),
        card_id: Set(m.as_ref().map_or(c.id, |m| m.card_id)),
        direction: Set(t.direction.clone()),
        identity: Set(queue::identity(&c)),
        study_key: Set(t.study_key.clone()),
        parameter_version: Set(profile.version()),
        stability: Set(result.stability),
        difficulty: Set(result.difficulty),
        step: Set(result.step),
        reviews: Set(t.version + 1),
        lapses: Set(m.as_ref().map_or(0, |m| m.lapses) + i32::from(input.rating == 1)),
        due_at: Set(result.due_at),
        reviewed_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            memory::Column::UserId,
            memory::Column::StudyKey,
            memory::Column::Direction,
        ])
        .update_columns([
            memory::Column::CardId,
            memory::Column::Identity,
            memory::Column::StudyKey,
            memory::Column::ParameterVersion,
            memory::Column::Stability,
            memory::Column::Difficulty,
            memory::Column::Step,
            memory::Column::Reviews,
            memory::Column::Lapses,
            memory::Column::DueAt,
            memory::Column::ReviewedAt,
        ])
        .to_owned(),
    )
    .exec(&txn)
    .await?;
    let snapshot = t.snapshot.clone();
    let mut saved: attempts::ActiveModel = t.into();
    saved.delta_days =
        Set(Some(m.as_ref().map_or(0, |m| {
            (now - m.reviewed_at).num_days().max(0) as i32
        })));
    saved.snapshot = Set(Some(match snapshot {
        Some(value) => value,
        None => serde_json::to_value(dto::Snapshot {
            word: c.word,
            meaning: c.meaning,
            ipa: c.ipa,
            locale: c.locale,
            part_of_speech: c.part_of_speech,
            topics: c.topics,
            examples: c.examples,
        })
        .map_err(|e| AppError::Internal(e.to_string()))?,
    }));
    saved.reviewed_at = Set(Some(now));
    saved.rating = Set(Some(i32::from(input.rating)));
    saved.due_at = Set(Some(result.due_at));
    // Return the persisted timestamp: PostgreSQL stores microseconds, while
    // the scheduler's in-memory timestamp can contain nanoseconds.
    let saved = saved.update(&txn).await?;
    txn.commit().await?;
    Ok(dto::GradeResult {
        rating: i32::from(input.rating),
        due_at: saved.due_at,
    })
}
