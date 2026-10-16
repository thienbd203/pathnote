use super::*;
pub async fn create(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    mut input: Definition,
    bulk: bool,
) -> Result<CreateResource, AppError> {
    notes::access(db, user, session, false).await?;
    input.validate(bulk)?;
    if input.revision.is_some() {
        return Err(invalid());
    }
    let questions = input
        .questions
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let (txn, _) = lock(db, user, session).await?;
    let models = questions
        .into_iter()
        .enumerate()
        .map(|(i, question)| exercises::ActiveModel {
            session_id: Set(session),
            user_id: Set(user),
            title: Set(input
                .titles
                .as_ref()
                .map_or_else(|| input.title.clone(), |titles| titles[i].clone())),
            question: Set(question),
            revision: Set(1),
            ..Default::default()
        });
    let ids = exercises::Entity::insert_many(models)
        .exec_with_returning_keys(&txn)
        .await?;
    txn.commit().await?;
    Ok(if bulk {
        CreateResource::Many { ids }
    } else {
        CreateResource::One { id: ids[0] }
    })
}
