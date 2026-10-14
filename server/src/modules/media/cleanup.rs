//! Only unregistered storage files are orphans. Unused media rows are retained for editor undo.
use crate::{entities::media, AppError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use std::{
    fs::{File, TryLockError},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
pub const GRACE: Duration = Duration::from_secs(24 * 60 * 60);
#[derive(Default, Serialize)]
pub struct Report {
    pub scanned: u64,
    pub removed: u64,
}
fn io(e: std::io::Error) -> AppError {
    AppError::Internal(format!("Orphan media cleanup: {e}"))
}
struct Candidate {
    key: String,
    path: PathBuf,
    lease: File,
}
async fn flush(
    db: &DatabaseConnection,
    batch: &mut Vec<Candidate>,
    report: &mut Report,
) -> Result<(), AppError> {
    let registered: std::collections::HashSet<_> = media::Entity::find()
        .filter(media::Column::StorageKey.is_in(batch.iter().map(|f| f.key.clone())))
        .all(db)
        .await?
        .into_iter()
        .map(|m| m.storage_key)
        .collect();
    for candidate in batch.drain(..) {
        if !registered.contains(&candidate.key) {
            match tokio::fs::remove_file(&candidate.path).await {
                Ok(()) => report.removed += 1,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(e)),
            }
        }
        drop(candidate.lease);
    }
    Ok(())
}
/// OS locks skip active uploads even if they outlive the age threshold; no DB row locks are held.
pub async fn clean(
    db: &DatabaseConnection,
    root: &Path,
    grace: Duration,
) -> Result<Report, AppError> {
    let mut report = Report::default();
    let mut directory = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(io(e)),
    };
    let cutoff = SystemTime::now()
        .checked_sub(grace)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut batch = Vec::new();
    while let Some(entry) = directory.next_entry().await.map_err(io)? {
        report.scanned += 1;
        if !entry.file_type().await.map_err(io)?.is_file() {
            continue;
        }
        let Some(key) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if uuid::Uuid::parse_str(&key)
            .ok()
            .is_none_or(|id| id.to_string() != key)
        {
            continue;
        }
        let target = entry.path();
        let lease = tokio::task::spawn_blocking(move || {
            let file = match File::options().read(true).write(true).open(&target) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(io(e)),
            };
            match file.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => return Ok(None),
                Err(TryLockError::Error(e)) => return Err(io(e)),
            }
            if file.metadata().map_err(io)?.modified().map_err(io)? > cutoff {
                return Ok(None);
            }
            Ok(Some(Candidate {
                key,
                path: target,
                lease: file,
            }))
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
        if let Some(file) = lease {
            batch.push(file);
        }
        if batch.len() == 100 {
            flush(db, &mut batch, &mut report).await?;
        }
    }
    if !batch.is_empty() {
        flush(db, &mut batch, &mut report).await?;
    }
    Ok(report)
}
