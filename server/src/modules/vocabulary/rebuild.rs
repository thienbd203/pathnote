use crate::{
    entities::{notes, places, sessions},
    modules::{notes::service as note_service, places::service as place_service},
    AppError,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
/// Backfill existing notes in small pages. Saving new notes already synchronizes atomically.
pub async fn rebuild(db: &DatabaseConnection) -> Result<u64, AppError> {
    let mut cursor = 0;
    let mut count = 0;
    loop {
        let ids: Vec<i64> = notes::Entity::find()
            .select_only()
            .column(notes::Column::SessionId)
            .filter(notes::Column::SessionId.gt(cursor))
            .order_by_asc(notes::Column::SessionId)
            .limit(100)
            .into_tuple()
            .all(db)
            .await?;
        if ids.is_empty() {
            break;
        }
        for id in ids {
            cursor = id;
            let Some(session) = sessions::Entity::find_by_id(id).one(db).await? else {
                continue;
            };
            let Some(place) = places::Entity::find_by_id(session.place_id).one(db).await? else {
                continue;
            };
            match note_service::access(db, place.user_id, id, true).await {
                Ok(_) => {}
                Err(AppError::NotFound(_)) => continue,
                Err(error) => return Err(error),
            }
            let Some(snapshot) = notes::Entity::find_by_id(id).one(db).await? else {
                continue;
            };
            let prepared = super::sync::prepare(&snapshot.content)?;
            let txn = place_service::write_tx(db, place.user_id, place.id).await?;
            note_service::access(&txn, place.user_id, id, true).await?;
            if let Some(note) = notes::Entity::find_by_id(id).one(&txn).await? {
                if note.revision != snapshot.revision {
                    txn.rollback().await?;
                    continue;
                }
                super::sync::sync(&txn, id, prepared).await?;
                count += 1;
            }
            txn.commit().await?;
        }
    }
    Ok(count)
}
