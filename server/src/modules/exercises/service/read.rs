use super::*;
use sea_orm::{Condition, QuerySelect};
use std::collections::HashMap;

pub async fn read(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    input: ReadQuery,
) -> Result<Vec<ExerciseResource>, AppError> {
    let (_, place) = notes::access(db, user, session, false).await?;
    let ids = input.validate()?;
    let models = exercises::Entity::find()
        .filter(exercises::Column::SessionId.eq(session))
        .filter(exercises::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await?;
    if models.is_empty() {
        return Ok(Vec::new());
    }
    // PostgreSQL DISTINCT ON bounds the result to one current, personal attempt per exercise.
    let revisions = models.iter().fold(Condition::any(), |condition, model| {
        condition.add(
            Condition::all()
                .add(attempts::Column::ExerciseId.eq(model.id))
                .add(attempts::Column::Revision.eq(model.revision)),
        )
    });
    let latest = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(user))
        .filter(revisions)
        .distinct_on([(attempts::Entity, attempts::Column::ExerciseId)])
        .order_by_asc(attempts::Column::ExerciseId)
        .order_by_desc(attempts::Column::Id)
        .all(db)
        .await?;
    let latest: HashMap<_, _> = latest.into_iter().map(|a| (a.exercise_id, a)).collect();
    let mut models: HashMap<_, _> = models.into_iter().map(|m| (m.id, m)).collect();
    // Omit missing / foreign-session IDs alike; no information about another session is exposed.
    ids.into_iter()
        .filter_map(|id| models.remove(&id))
        .map(|model| {
            let a = latest.get(&model.id);
            public_exercise(model, a, user, place.user_id)
        })
        .collect()
}
