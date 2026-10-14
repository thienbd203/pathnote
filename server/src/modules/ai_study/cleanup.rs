use crate::{
    entities::{ai_imports as imports, exercises, places, sessions},
    AppError,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
};
/// Delete only batches never included in a successfully saved note. Parent lock serializes note attachment.
pub async fn clean(
    db: &DatabaseConnection,
    before: chrono::DateTime<chrono::FixedOffset>,
) -> Result<u64, AppError> {
    let pending: Vec<(uuid::Uuid, i64, i64, serde_json::Value)> = imports::Entity::find()
        .select_only()
        .columns([
            imports::Column::Id,
            imports::Column::SessionId,
            imports::Column::UserId,
            imports::Column::ExerciseIds,
        ])
        .filter(imports::Column::Attached.eq(false))
        .filter(imports::Column::CreatedAt.lt(before))
        .limit(100)
        .into_tuple()
        .all(db)
        .await?;
    let mut removed = 0;
    for (key, session_id, user_id, data) in pending {
        let ids: Vec<i64> =
            serde_json::from_value(data).map_err(|e| AppError::Internal(e.to_string()))?;
        let txn = db.begin().await?;
        let session = sessions::Entity::find_by_id(session_id)
            .one(&txn)
            .await?
            .ok_or_else(|| AppError::Internal("Import missing parent".into()))?;
        places::Entity::find_by_id(session.place_id)
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or_else(|| AppError::Internal("Import missing place".into()))?;
        if let Some(id) = imports::Entity::find_by_id(key)
            .filter(imports::Column::Attached.eq(false))
            .filter(imports::Column::CreatedAt.lt(before))
            .select_only()
            .column(imports::Column::Id)
            .into_tuple::<uuid::Uuid>()
            .one(&txn)
            .await?
        {
            exercises::Entity::delete_many()
                .filter(exercises::Column::SessionId.eq(session_id))
                .filter(exercises::Column::UserId.eq(user_id))
                .filter(exercises::Column::Id.is_in(ids))
                .exec(&txn)
                .await?;
            imports::Entity::delete_by_id(id).exec(&txn).await?;
            removed += 1;
        }
        txn.commit().await?;
    }
    Ok(removed)
}
