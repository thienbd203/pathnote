use super::Kind;
use crate::{
    entities::media,
    modules::{notes::service as notes, places::service as places},
    state::AppState,
    AppError,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
#[derive(Serialize)]
pub struct MediaResource {
    pub id: i64,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
}
impl From<media::Model> for MediaResource {
    fn from(m: media::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            mime_type: m.mime_type,
            size: m.size,
        }
    }
}
fn io(e: std::io::Error) -> AppError {
    AppError::Internal(format!("Private media storage: {e}"))
}
fn path(state: &AppState, key: &str) -> Result<std::path::PathBuf, AppError> {
    uuid::Uuid::parse_str(key).map_err(|_| AppError::Internal("Invalid storage key".into()))?;
    Ok(std::path::Path::new(&state.config.media_root).join(key))
}
pub async fn create(
    state: &AppState,
    user: i64,
    id: i64,
    kind: Kind,
    name: String,
    data: Vec<u8>,
) -> Result<MediaResource, AppError> {
    notes::access(state.db.as_ref(), user, id, false).await?;
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
    let (_, place) = notes::access(state.db.as_ref(), user, id, false).await?;
    let txn = state.db.begin().await?;
    places::lock_for_write(&txn, place.user_id, place.id).await?;
    notes::access(&txn, user, id, false).await?;
    let key = uuid::Uuid::new_v4().to_string();
    let target = path(state, &key)?;
    let mut directory = tokio::fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    directory.mode(0o700);
    directory
        .create(&state.config.media_root)
        .await
        .map_err(io)?;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = async {
        let mut file = options.open(&target).await.map_err(io)?;
        file.write_all(&data).await.map_err(io)?;
        file.sync_all().await.map_err(io)?;
        let model = media::ActiveModel {
            session_id: Set(id),
            kind: Set(kind.field().into()),
            name: Set(name),
            storage_key: Set(key),
            mime_type: Set(mime),
            size: Set(data.len() as i64),
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
        let _ = tokio::fs::remove_file(target).await;
    }
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
pub async fn bytes(state: &AppState, model: &media::Model) -> Result<Vec<u8>, AppError> {
    tokio::fs::read(path(state, &model.storage_key)?)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound("Không tìm thấy tài liệu.".into())
            } else {
                io(e)
            }
        })
}
