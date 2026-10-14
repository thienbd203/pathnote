//! Atomic soft deletion. All writers acquire place locks first, ordered by ID.
use super::{
    courses::service as course_service, places::service as place_service,
    sessions::service as session_service,
};
use crate::{
    access_token::CurrentUser,
    entities::{courses, places, sessions},
    state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use serde::Deserialize;
use std::collections::BTreeSet;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    ids: Vec<i64>,
}
#[derive(Clone, Copy)]
enum Kind {
    Place,
    Course,
    Session,
}
async fn parents<C: ConnectionTrait>(
    db: &C,
    user: i64,
    kind: Kind,
    ids: &[i64],
) -> Result<BTreeSet<i64>, AppError> {
    let (count, parents) = match kind {
        Kind::Place => {
            let rows = place_service::owned(user)
                .filter(places::Column::Id.is_in(ids.to_vec()))
                .all(db)
                .await?;
            (rows.len(), rows.into_iter().map(|r| r.id).collect())
        }
        Kind::Course => {
            let rows = course_service::owned(user)
                .filter(courses::Column::Id.is_in(ids.to_vec()))
                .all(db)
                .await?;
            (rows.len(), rows.into_iter().map(|r| r.place_id).collect())
        }
        Kind::Session => {
            let rows = session_service::owned(user)
                .filter(sessions::Column::Id.is_in(ids.to_vec()))
                .all(db)
                .await?;
            (rows.len(), rows.into_iter().map(|r| r.place_id).collect())
        }
    };
    if count != ids.len() {
        return Err(AppError::NotFound(
            "Không tìm thấy một hoặc nhiều mục đã chọn.".into(),
        ));
    }
    Ok(parents)
}
async fn remove(
    db: &DatabaseConnection,
    user: i64,
    kind: Kind,
    input: Input,
) -> Result<(), AppError> {
    let ids = input.ids;
    if ids.is_empty()
        || ids.len() > 100
        || ids.iter().any(|id| *id <= 0)
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(AppError::validation(
            "ids",
            "Chọn từ 1 đến 100 ID hợp lệ, không lặp.",
        ));
    }
    let place_ids = parents(db, user, kind, &ids).await?;
    let txn = db.begin().await?;
    for id in place_ids {
        place_service::lock_for_write(&txn, user, id).await?;
    }
    parents(&txn, user, kind, &ids).await?;
    let now = chrono::Utc::now().fixed_offset();
    let session_filter = match kind {
        Kind::Place => sessions::Column::PlaceId.is_in(ids.clone()),
        Kind::Course => sessions::Column::CourseId.is_in(ids.clone()),
        Kind::Session => sessions::Column::Id.is_in(ids.clone()),
    };
    sessions::Entity::update_many()
        .col_expr(sessions::Column::DeletedAt, Expr::value(now))
        .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
        .filter(session_filter)
        .filter(sessions::Column::DeletedAt.is_null())
        .exec(&txn)
        .await?;
    if matches!(kind, Kind::Place | Kind::Course) {
        let filter = match kind {
            Kind::Place => courses::Column::PlaceId.is_in(ids.clone()),
            _ => courses::Column::Id.is_in(ids.clone()),
        };
        courses::Entity::update_many()
            .col_expr(courses::Column::DeletedAt, Expr::value(now))
            .col_expr(courses::Column::UpdatedAt, Expr::value(now))
            .filter(filter)
            .filter(courses::Column::DeletedAt.is_null())
            .exec(&txn)
            .await?;
    }
    if matches!(kind, Kind::Place) {
        places::Entity::update_many()
            .col_expr(places::Column::DeletedAt, Expr::value(now))
            .col_expr(places::Column::UpdatedAt, Expr::value(now))
            .filter(places::Column::Id.is_in(ids))
            .filter(places::Column::DeletedAt.is_null())
            .exec(&txn)
            .await?;
    }
    txn.commit().await?;
    Ok(())
}
async fn handler(
    s: AppState,
    u: CurrentUser,
    kind: Kind,
    body: Result<Json<Input>, JsonRejection>,
) -> Result<StatusCode, AppError> {
    let Json(input) =
        body.map_err(|_| AppError::validation("ids", "Danh sách ID không hợp lệ."))?;
    remove(&s.db, u.id, kind, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn places(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<Json<Input>, JsonRejection>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Place, b).await
}
pub async fn courses(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<Json<Input>, JsonRejection>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Course, b).await
}
pub async fn sessions(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<Json<Input>, JsonRejection>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Session, b).await
}
