use super::{self as service};
use crate::{
    config::Config,
    entities::{notes, sessions, users},
    modules::{notes::dto::*, places::service as places},
    AppError,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect,
};
pub async fn config(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    config: &Config,
    enrichment_enabled: bool,
) -> Result<CollaborationConfigResource, AppError> {
    service::access(db, user, id, false).await?;
    if config.collaboration_url.is_none()
        && notes::Entity::find_by_id(id)
            .select_only()
            .expr(notes::Column::YjsState.is_not_null())
            .into_tuple::<bool>()
            .one(db)
            .await?
            .unwrap_or(false)
    {
        return Err(AppError::BadRequest(
            "Realtime chưa được cấu hình cho ghi chép này.".into(),
        ));
    }
    Ok(CollaborationConfigResource {
        enrichment_enabled,
        enabled: config.collaboration_url.is_some(),
        url: config.collaboration_url.clone(),
        document_name: format!("session-{id}"),
    })
}
pub async fn authorize(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    schema_version: Option<i32>,
) -> Result<CollaborationUserResource, AppError> {
    service::access(db, user, id, false).await?;
    super::super::schema::check_client(schema_version.unwrap_or(0))?;
    let user = users::Entity::find_by_id(user)
        .one(db)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    Ok(CollaborationUserResource {
        id: user.id,
        name: user.name,
    })
}
async fn active_session<C: ConnectionTrait>(db: &C, id: i64) -> Result<sessions::Model, AppError> {
    service::active()
        .filter(sessions::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy ghi chép.".into()))
}
pub async fn fetch(db: &DatabaseConnection, id: i64) -> Result<StateResource, AppError> {
    active_session(db, id).await?;
    let loaded = notes::Entity::find_by_id(id).one(db).await?;
    let state = loaded
        .as_ref()
        .and_then(|n| n.yjs_state.as_ref())
        .map(|bytes| STANDARD.encode(bytes));
    Ok(StateResource {
        note: service::resource(id, loaded),
        state,
    })
}
pub async fn store(
    db: &DatabaseConnection,
    id: i64,
    input: SaveState,
) -> Result<NoteResource, AppError> {
    let session = active_session(db, id).await?;
    if !(0..i32::MAX).contains(&input.base_revision)
        || input.state.len() > super::super::limits::YJS_BASE64_BYTES
    {
        return Err(AppError::validation(
            "state",
            "Trạng thái đồng bộ không hợp lệ.",
        ));
    }
    let bytes = STANDARD
        .decode(&input.state)
        .map_err(|_| AppError::validation("state", "Trạng thái đồng bộ không hợp lệ."))?;
    if bytes.is_empty() || bytes.len() > super::super::limits::YJS_STATE_BYTES {
        return Err(AppError::validation(
            "state",
            "Trạng thái đồng bộ quá lớn hoặc rỗng.",
        ));
    }
    if let Some(version) = input.schema_version {
        super::super::schema::check_client(version)?;
    }
    let version = input
        .schema_version
        .unwrap_or(crate::modules::notes::schema::LEGACY_NOTE_SCHEMA_VERSION);
    let references = crate::modules::notes::document::validate(&input.content, version)?;
    let vocabulary = crate::modules::vocabulary::sync::prepare(&input.content)?;
    let place = crate::entities::places::Entity::find_by_id(session.place_id)
        .one(db)
        .await?
        .ok_or(AppError::Forbidden)?;
    let txn = places::write_tx(db, place.user_id, place.id).await?;
    active_session(&txn, id).await?;
    let loaded = notes::Entity::find_by_id(id).one(&txn).await?;
    let unchanged = loaded
        .as_ref()
        .is_some_and(|n| n.yjs_state.as_ref() == Some(&bytes) && n.content == input.content);
    let current = service::resource(id, loaded);
    let version =
        super::super::schema::write_version(input.schema_version, &current, &input.content)?;
    service::validate_references(&txn, id, &references).await?;
    if unchanged {
        return Ok(current);
    }
    if current.revision != input.base_revision {
        return Err(AppError::ConflictWith {
            message: "Ghi chép đã thay đổi.".into(),
            data: serde_json::to_value(current).map_err(|e| AppError::Internal(e.to_string()))?,
        });
    }
    let note =
        service::persist::persist(&txn, id, input.content, Some(bytes), version, &current).await?;
    crate::modules::ai_study::service::attach(&txn, id, &references).await?;
    crate::modules::vocabulary::sync::sync(&txn, id, vocabulary).await?;
    txn.commit().await?;
    Ok(note)
}
