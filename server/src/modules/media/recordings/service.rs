use super::dto::{PlaybackResource, Progress, RecordingResource};
use crate::{
    entities::{media, media_progress},
    modules::notes::service as notes,
    state::AppState,
    AppError,
};
use sea_orm::{
    sea_query::{Expr, OnConflict, Query},
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use std::collections::HashMap;
const TYPES: [&str; 3] = ["audio/mpeg", "audio/mp4", "video/mp4"];
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
    // Per-user derived state: authorize in the same statement as the upsert,
    // without serializing note saves/uploads on the place row.
    let allowed = notes::accessible(user, false)
        .filter(crate::entities::sessions::Column::Id.eq(session))
        .select_only()
        .column(crate::entities::sessions::Column::Id)
        .into_query();
    let source = Query::select()
        .expr(Expr::val(user))
        .column(media::Column::Id)
        .expr(Expr::val(input.position_seconds))
        .expr(Expr::val(chrono::Utc::now().fixed_offset()))
        .from(media::Entity)
        .and_where(media::Column::Id.eq(id))
        .and_where(media::Column::SessionId.eq(session))
        .and_where(media::Column::Kind.eq("file"))
        .and_where(media::Column::MimeType.is_in(TYPES))
        .and_where(Expr::exists(allowed))
        .to_owned();
    let query = Query::insert()
        .into_table(media_progress::Entity)
        .columns([
            media_progress::Column::UserId,
            media_progress::Column::MediaId,
            media_progress::Column::PositionSeconds,
            media_progress::Column::UpdatedAt,
        ])
        .select_from(source)
        .map_err(|e| AppError::Internal(e.to_string()))?
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
        .to_owned();
    let affected = state
        .db
        .execute(state.db.get_database_backend().build(&query))
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("Không tìm thấy file nghe/xem.".into()));
    }
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
