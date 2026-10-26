use super::{resource, NoteResource};
use crate::{
    entities::{notes, sessions},
    AppError,
};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    Set,
};
use serde_json::Value;

/// Call only after permission, schema, references and revision checks under the place lock.
/// REST and bridge share persistence; callers own projection sync and transaction commit.
pub(super) async fn persist(
    txn: &DatabaseTransaction,
    id: i64,
    content: Value,
    yjs: Option<Vec<u8>>,
    version: i32,
    current: &NoteResource,
) -> Result<NoteResource, AppError> {
    let now = chrono::Utc::now().fixed_offset();
    let next = notes::ActiveModel {
        session_id: Set(id),
        content: Set(content),
        yjs_state: Set(yjs),
        schema_version: Set(version),
        revision: Set(current.revision + 1),
        saved_at: Set(now),
    };
    let saved = if current.saved_at.is_none() {
        next.insert(txn).await?
    } else {
        next.update(txn).await?
    };
    sessions::Entity::update_many()
        .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
        .filter(sessions::Column::Id.eq(id))
        .exec(txn)
        .await?;
    Ok(resource(id, Some(saved)))
}
