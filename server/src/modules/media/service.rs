use super::dto::{Kind, MediaResource};
use crate::{
    entities::{media, places as place_model, sessions},
    modules::{notes::service as notes, places::service as places},
    state::AppState,
    AppError,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
fn io(e: std::io::Error) -> AppError {
    AppError::Internal(format!("Private media storage: {e}"))
}
fn path(state: &AppState, key: &str) -> Result<std::path::PathBuf, AppError> {
    uuid::Uuid::parse_str(key).map_err(|_| AppError::Internal("Invalid storage key".into()))?;
    Ok(std::path::Path::new(&state.config.media_root).join(key))
}
pub(super) async fn create(
    state: &AppState,
    user: i64,
    kind: Kind,
    name: String,
    data: Vec<u8>,
    access: (sessions::Model, place_model::Model),
) -> Result<MediaResource, AppError> {
    let (session, place) = access;
    let id = session.id;
    let mut name = name
        .chars()
        .map(|c| {
            if c.is_control() || ['/', '\\', '"'].contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect::<String>();
    if name.chars().count() > 200 {
        if kind == Kind::File {
            let ext = name.rsplit('.').next().unwrap_or("").to_owned();
            let prefix = name
                .chars()
                .take(200usize.saturating_sub(ext.chars().count() + 1))
                .collect::<String>();
            name = format!("{prefix}.{ext}");
        } else {
            name = name.chars().take(200).collect();
        }
    }
    let (data, mime, name) = tokio::task::spawn_blocking(move || {
        let mime = super::validation::inspect(kind, &name, &data)?;
        Ok::<_, AppError>((data, mime, name))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    let key = uuid::Uuid::new_v4().to_string();
    let target = path(state, &key)?;
    let size = data.len() as i64;
    let root = std::path::PathBuf::from(&state.config.media_root);
    let disk_target = target.clone();
    // File I/O and fsync run before BEGIN. Hold the OS file lock until the DB outcome is known.
    let lease = tokio::task::spawn_blocking(move || super::storage::write(root, disk_target, data))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    let result = async {
        let txn = places::write_tx(&state.db, place.user_id, place.id).await?;
        notes::access(&txn, user, id, false).await?;
        let model = media::ActiveModel {
            session_id: Set(id),
            kind: Set(kind.field().into()),
            name: Set(name),
            storage_key: Set(key.clone()),
            mime_type: Set(mime),
            size: Set(size),
            created_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        txn.commit().await?;
        Ok::<_, AppError>(MediaResource::from(model))
    }
    .await;
    if result.is_err() {
        // A COMMIT error can mean its acknowledgement was lost. Never remove a file
        // if its record exists, or if the DB cannot confirm the outcome.
        match media::Entity::find()
            .filter(media::Column::StorageKey.eq(&key))
            .one(state.db.as_ref())
            .await
        {
            Ok(None) => {
                if let Err(e) = tokio::fs::remove_file(&target).await {
                    tracing::warn!(%e, "Không xóa được upload thất bại; cleanup sẽ thử lại");
                }
            }
            Ok(Some(_)) => {}
            Err(e) => {
                tracing::warn!(%e, "Chưa xác định được kết quả upload; giữ file để cleanup kiểm tra lại")
            }
        }
    }
    drop(lease);
    result
}
pub async fn get(
    state: &AppState,
    user: i64,
    id: i64,
    kind: Kind,
    media_id: i64,
) -> Result<media::Model, AppError> {
    notes::access(state.db.as_ref(), user, id, false).await?;
    media::Entity::find_by_id(media_id)
        .filter(media::Column::SessionId.eq(id))
        .filter(media::Column::Kind.eq(kind.field()))
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy tài liệu.".into()))
}
pub async fn open(state: &AppState, model: &media::Model) -> Result<tokio::fs::File, AppError> {
    tokio::fs::File::open(path(state, &model.storage_key)?)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound("Không tìm thấy tài liệu.".into())
            } else {
                io(e)
            }
        })
}
