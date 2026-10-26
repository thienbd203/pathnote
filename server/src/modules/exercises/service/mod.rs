mod create;
mod read;
use super::dto::*;
use crate::{
    entities::{exercise_attempts as attempts, exercises},
    modules::{notes::service as notes, places::service as place_service},
    AppError,
};
pub use create::create;
pub(in crate::modules) use create::insert_many;
pub use read::read;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, Set,
};

fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy câu hỏi.".into())
}
fn conflict() -> AppError {
    AppError::Conflict("Đề hoặc lần thử đã thay đổi. Hãy tải lại bài tập.".into())
}
async fn lock(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
) -> Result<(DatabaseTransaction, i64), AppError> {
    let (_, place) = notes::access(db, user, session, false).await?;
    let txn = place_service::write_tx(db, place.user_id, place.id).await?;
    let (_, place) = notes::access(&txn, user, session, false).await?;
    Ok((txn, place.user_id))
}
async fn get<C: ConnectionTrait>(
    db: &C,
    session: i64,
    id: i64,
) -> Result<exercises::Model, AppError> {
    exercises::Entity::find_by_id(id)
        .filter(exercises::Column::SessionId.eq(session))
        .one(db)
        .await?
        .ok_or_else(missing)
}
fn question(model: &exercises::Model) -> Result<Question, AppError> {
    serde_json::from_value(model.question.clone()).map_err(|e| AppError::Internal(e.to_string()))
}
fn manage(model: &exercises::Model, user: i64, owner: i64) -> Result<(), AppError> {
    if user == owner || user == model.user_id {
        Ok(())
    } else {
        Err(missing())
    }
}
async fn latest<C: ConnectionTrait>(
    db: &C,
    user: i64,
    model: &exercises::Model,
) -> Result<Option<attempts::Model>, AppError> {
    Ok(attempts::Entity::find()
        .filter(attempts::Column::ExerciseId.eq(model.id))
        .filter(attempts::Column::UserId.eq(user))
        .filter(attempts::Column::Revision.eq(model.revision))
        .order_by_desc(attempts::Column::Id)
        .one(db)
        .await?)
}
fn result(model: &attempts::Model) -> Result<Option<ResultData>, AppError> {
    model
        .result
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))
}
fn attempt(model: &attempts::Model, q: &Question) -> Result<AttemptResource, AppError> {
    let result = result(model)?;
    let completed = result.as_ref().is_some_and(|r| r.r#final);
    let results = result
        .into_iter()
        .map(|result| AttemptResultResource {
            result,
            answers: completed.then(|| q.answers().to_vec()),
            explanation: completed.then(|| q.explanation().to_owned()),
        })
        .collect();
    Ok(AttemptResource {
        id: model.id,
        revision: model.revision,
        position: usize::from(completed),
        completed,
        questions: vec![q.public()],
        results,
    })
}
fn public_exercise(
    model: exercises::Model,
    latest: Option<&attempts::Model>,
    user: i64,
    owner: i64,
) -> Result<ExerciseResource, AppError> {
    let q = question(&model)?;
    Ok(ExerciseResource {
        id: model.id,
        title: model.title,
        revision: model.revision,
        can_manage: user == owner || user == model.user_id,
        questions: vec![q.public()],
        attempt: latest.map(|a| attempt(a, &q)).transpose()?,
    })
}
pub async fn show(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    id: i64,
    definition: bool,
) -> Result<ShowResource, AppError> {
    let (_, place) = notes::access(db, user, session, false).await?;
    let model = get(db, session, id).await?;
    if definition {
        let q = question(&model)?;
        manage(&model, user, place.user_id)?;
        Ok(ShowResource::Definition(DefinitionResource {
            title: model.title,
            questions: vec![q],
            revision: model.revision,
        }))
    } else {
        let a = latest(db, user, &model).await?;
        Ok(ShowResource::Exercise(public_exercise(
            model,
            a.as_ref(),
            user,
            place.user_id,
        )?))
    }
}
pub async fn update(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    id: i64,
    mut input: Definition,
) -> Result<IdResource, AppError> {
    notes::access(db, user, session, false).await?;
    input.validate(false)?;
    let q =
        serde_json::to_value(&input.questions[0]).map_err(|e| AppError::Internal(e.to_string()))?;
    let (txn, owner) = lock(db, user, session).await?;
    let model = get(&txn, session, id).await?;
    manage(&model, user, owner)?;
    if input.revision != Some(model.revision) {
        return Err(conflict());
    }
    let revision = model.revision.checked_add(1).ok_or_else(conflict)?;
    let mut model: exercises::ActiveModel = model.into();
    model.title = Set(input.title);
    model.question = Set(q);
    model.revision = Set(revision);
    model.update(&txn).await?;
    txn.commit().await?;
    Ok(IdResource { id })
}
pub async fn start(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    id: i64,
    input: Start,
) -> Result<AttemptResource, AppError> {
    let (txn, _) = lock(db, user, session).await?;
    let model = get(&txn, session, id).await?;
    let old = latest(&txn, user, &model).await?;
    let a = match old.filter(|_| !input.restart) {
        Some(existing) => existing,
        None => {
            attempts::ActiveModel {
                exercise_id: Set(id),
                user_id: Set(user),
                revision: Set(model.revision),
                result: Set(None),
                ..Default::default()
            }
            .insert(&txn)
            .await?
        }
    };
    let value = attempt(&a, &question(&model)?)?;
    txn.commit().await?;
    Ok(value)
}
pub async fn answer(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    id: i64,
    aid: i64,
    input: Answer,
) -> Result<AttemptResource, AppError> {
    if input.position != 0
        || input.attempt > 1
        || input.answer.trim().is_empty()
        || input.answer.chars().count() > 4000
    {
        return Err(AppError::validation("answer", "Câu trả lời không hợp lệ."));
    }
    let (txn, _) = lock(db, user, session).await?;
    let model = get(&txn, session, id).await?;
    let a = attempts::Entity::find_by_id(aid)
        .filter(attempts::Column::ExerciseId.eq(id))
        .filter(attempts::Column::UserId.eq(user))
        .one(&txn)
        .await?
        .ok_or_else(missing)?;
    if a.revision != model.revision
        || latest(&txn, user, &model)
            .await?
            .is_none_or(|m| m.id != aid)
    {
        return Err(conflict());
    }
    let q = question(&model)?;
    let previous = result(&a)?;
    if let Some(p) = &previous {
        if p.tries == input.attempt + 1 && p.answer == input.answer {
            let value = attempt(&a, &q)?;
            txn.commit().await?;
            return Ok(value);
        }
        if p.r#final {
            return Err(conflict());
        }
    }
    if previous.as_ref().map_or(0, |p| p.tries) != input.attempt {
        return Err(conflict());
    }
    if input.answer.chars().count() > q.answer_limit() {
        return Err(AppError::validation("answer", "Câu trả lời quá dài."));
    }
    let correct = q.grade(&input.answer)?;
    let r = ResultData {
        position: 0,
        answer: input.answer,
        tries: input.attempt + 1,
        correct,
        r#final: correct || input.attempt == 1,
    };
    let mut a: attempts::ActiveModel = a.into();
    a.result = Set(Some(
        serde_json::to_value(r).map_err(|e| AppError::Internal(e.to_string()))?,
    ));
    let a = a.update(&txn).await?;
    let value = attempt(&a, &q)?;
    txn.commit().await?;
    Ok(value)
}
/// References are validated under the note's parent lock, for REST and Yjs saves.
pub(in crate::modules) async fn references<C: ConnectionTrait>(
    db: &C,
    session: i64,
    ids: &[i64],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let found = exercises::Entity::find()
        .filter(exercises::Column::SessionId.eq(session))
        .filter(exercises::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await?;
    if ids.iter().any(|id| !found.iter().any(|q| q.id == *id)) {
        return Err(AppError::validation(
            "content",
            "Câu hỏi không thuộc buổi học này.",
        ));
    }
    Ok(())
}
