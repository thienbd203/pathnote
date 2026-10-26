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
    let rows = questions.into_iter().enumerate().map(|(i, question)| {
        (
            input
                .titles
                .as_ref()
                .map_or_else(|| input.title.clone(), |titles| titles[i].clone()),
            question,
        )
    });
    let ids = insert_many(&txn, user, session, rows).await?;
    txn.commit().await?;
    Ok(if bulk {
        CreateResource::Many { ids }
    } else {
        CreateResource::One { id: ids[0] }
    })
}

/// Callers validate and serialize questions before taking their parent write lock.
/// Keep input order: AI placeholders and bulk creation receipts refer to these IDs.
pub(in crate::modules) async fn insert_many<C: ConnectionTrait>(
    db: &C,
    user: i64,
    session: i64,
    rows: impl IntoIterator<Item = (String, serde_json::Value)>,
) -> Result<Vec<i64>, AppError> {
    let models: Vec<_> = rows
        .into_iter()
        .map(|(title, question)| exercises::ActiveModel {
            session_id: Set(session),
            user_id: Set(user),
            title: Set(title),
            question: Set(question),
            revision: Set(1),
            ..Default::default()
        })
        .collect();
    if models.is_empty() {
        return Ok(Vec::new());
    }
    Ok(exercises::Entity::insert_many(models)
        .exec_with_returning_keys(db)
        .await?)
}
