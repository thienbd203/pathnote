use super::dto::{PlaybackResource, Progress, RecordingResource};
use crate::{
    entities::{media, media_progress},
    modules::{notes::service as notes, places::service as places},
    state::AppState,
    AppError,
};
use sea_orm::{sea_query::OnConflict, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use std::collections::HashMap;
const TYPES: [&str; 2] = ["audio/mpeg", "video/mp4"];
async fn get(state: &AppState, user: i64, session: i64, id: i64) -> Result<media::Model, AppError> {
    let model =
        super::super::service::get(state, user, session, super::super::dto::Kind::File, id).await?;
    if !TYPES.contains(&model.mime_type.as_str()) {
        return Err(AppError::NotFound("Không tìm thấy file nghe/xem.".into()));
    }
    Ok(model)
}
pub async fn list(
    state: &AppState,
    user: i64,
    session: i64,
) -> Result<Vec<RecordingResource>, AppError> {
    notes::access(state.db.as_ref(), user, session, false).await?;
    let files = media::Entity::find()
        .filter(media::Column::SessionId.eq(session))
        .filter(media::Column::Kind.eq("file"))
        .filter(media::Column::MimeType.is_in(TYPES))
        .order_by_asc(media::Column::Id)
        .all(state.db.as_ref())
        .await?;
    let positions: HashMap<_, _> = if files.is_empty() {
        HashMap::new()
    } else {
        media_progress::Entity::find()
            .filter(media_progress::Column::UserId.eq(user))
            .filter(media_progress::Column::MediaId.is_in(files.iter().map(|m| m.id)))
            .all(state.db.as_ref())
            .await?
            .into_iter()
            .map(|p| (p.media_id, p.position_seconds))
            .collect()
    };
    Ok(files
        .into_iter()
        .map(|m| RecordingResource {
            position_seconds: positions.get(&m.id).copied().unwrap_or(0.0),
            id: m.id,
            name: m.name,
            mime_type: m.mime_type,
            size: m.size,
        })
        .collect())
}
pub async fn playback(
    state: &AppState,
    user: i64,
    session: i64,
    id: i64,
) -> Result<PlaybackResource, AppError> {
    get(state, user, session, id).await?;
    Ok(PlaybackResource {
        ticket: state.access_tokens.issue_playback(user, session, id)?,
    })
}
pub async fn stream(
    state: &AppState,
    session: i64,
    id: i64,
    ticket: &str,
) -> Result<(std::path::PathBuf, String), AppError> {
    let user = state.access_tokens.verify_playback(ticket, session, id)?;
    // A ticket does not bypass a deleted lesson or revoked share. Check each range.
    let model = get(state, user, session, id).await?;
    Ok((
        super::super::service::path(state, &model.storage_key)?,
        model.mime_type,
    ))
}
pub async fn progress(
    state: &AppState,
    user: i64,
    session: i64,
    id: i64,
    input: Progress,
) -> Result<Progress, AppError> {
    input.validate()?;
    let (_, place) = notes::access(state.db.as_ref(), user, session, false).await?;
    let txn = places::write_tx(state.db.as_ref(), place.user_id, place.id).await?;
    notes::access(&txn, user, session, false).await?;
    let model = media::Entity::find_by_id(id)
        .filter(media::Column::SessionId.eq(session))
        .filter(media::Column::Kind.eq("file"))
        .filter(media::Column::MimeType.is_in(TYPES))
        .one(&txn)
        .await?;
    if model.is_none() {
        return Err(AppError::NotFound("Không tìm thấy file nghe/xem.".into()));
    }
    media_progress::Entity::insert(media_progress::ActiveModel {
        user_id: Set(user),
        media_id: Set(id),
        position_seconds: Set(input.position_seconds),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    })
    .on_conflict(
        OnConflict::columns([
            media_progress::Column::UserId,
            media_progress::Column::MediaId,
        ])
        .update_columns([
            media_progress::Column::PositionSeconds,
            media_progress::Column::UpdatedAt,
        ])
        .to_owned(),
    )
    .exec(&txn)
    .await?;
    txn.commit().await?;
    Ok(input)
}

pub async fn read_progress(
    state: &AppState,
    user: i64,
    session: i64,
    id: i64,
) -> Result<Progress, AppError> {
    get(state, user, session, id).await?;
    let saved = media_progress::Entity::find_by_id((user, id))
        .one(state.db.as_ref())
        .await?;
    Ok(Progress {
        position_seconds: saved.map_or(0.0, |row| row.position_seconds),
    })
}
