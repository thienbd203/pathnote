use super::dto::{self, CreatePlace, ListPlaces, PageMeta, PlacePage, UpdatePlace};
use crate::{entities::places, AppError};
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set,
    TransactionTrait,
};

// One ownership scope is reused for reads, lists, updates and deletion.
fn owned(user_id: i64) -> Select<places::Entity> {
    places::Entity::find()
        .filter(places::Column::UserId.eq(user_id))
        .filter(places::Column::DeletedAt.is_null())
}
fn not_found() -> AppError {
    AppError::NotFound("Không tìm thấy nơi học.".into())
}

pub async fn get<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
) -> Result<places::Model, AppError> {
    owned(user_id)
        .filter(places::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(not_found)
}
pub async fn list(
    db: &DatabaseConnection,
    user_id: i64,
    input: ListPlaces,
) -> Result<PlacePage, AppError> {
    let size = input.page_size()?;
    let mut query = owned(user_id);
    if let Some(term) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let literal = term
            .to_lowercase()
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        query = query.filter(
            Expr::expr(Func::lower(Expr::col(places::Column::Name)))
                .like(LikeExpr::new(format!("%{literal}%")).escape('!')),
        );
    }
    let paginator = query.order_by_asc(places::Column::Id).paginate(db, size);
    let total = paginator.num_items().await?;
    let records = paginator.fetch_page(input.page - 1).await?;
    Ok(PlacePage {
        data: records.into_iter().map(Into::into).collect(),
        meta: PageMeta {
            current_page: input.page,
            last_page: total.div_ceil(size).max(1),
            per_page: size,
            total,
        },
    })
}
pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    input: CreatePlace,
) -> Result<places::Model, AppError> {
    let now = chrono::Utc::now().fixed_offset();
    Ok(places::ActiveModel {
        user_id: Set(user_id),
        name: Set(dto::validate_name(input.name)?),
        description: Set(dto::validate_description(input.description)?),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?)
}
pub async fn update(
    db: &DatabaseConnection,
    user_id: i64,
    id: i64,
    input: UpdatePlace,
) -> Result<places::Model, AppError> {
    // Check ownership before validating changes to an existing resource.
    get(db, user_id, id).await?;
    let mut changes: places::ActiveModel = Default::default();
    if let Some(name) = input.name {
        changes.name = Set(dto::validate_name(name.ok_or_else(|| {
            AppError::validation("name", "Tên nơi học không được null.")
        })?)?);
    }
    if let Some(description) = input.description {
        changes.description = Set(dto::validate_description(description)?);
    }
    changes.updated_at = Set(chrono::Utc::now().fixed_offset());
    // The UPDATE repeats the ownership/active predicates, so a concurrent delete
    // cannot update or return a record that has already been soft-deleted.
    places::Entity::update_many()
        .set(changes)
        .filter(places::Column::Id.eq(id))
        .filter(places::Column::UserId.eq(user_id))
        .filter(places::Column::DeletedAt.is_null())
        .exec_with_returning(db)
        .await?
        .into_iter()
        .next()
        .ok_or_else(not_found)
}
pub async fn lock_for_write(
    db: &DatabaseTransaction,
    user_id: i64,
    id: i64,
) -> Result<places::Model, AppError> {
    owned(user_id)
        .filter(places::Column::Id.eq(id))
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or_else(not_found)
}
pub async fn delete(db: &DatabaseConnection, user_id: i64, id: i64) -> Result<(), AppError> {
    let txn = db.begin().await?;
    lock_for_write(&txn, user_id, id).await?;
    let now = chrono::Utc::now().fixed_offset();
    use crate::entities::sessions;
    sessions::Entity::update_many()
        .col_expr(sessions::Column::DeletedAt, Expr::value(now))
        .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
        .filter(sessions::Column::PlaceId.eq(id))
        .filter(sessions::Column::DeletedAt.is_null())
        .exec(&txn)
        .await?;
    use crate::entities::courses;
    courses::Entity::update_many()
        .col_expr(courses::Column::DeletedAt, Expr::value(now))
        .col_expr(courses::Column::UpdatedAt, Expr::value(now))
        .filter(courses::Column::PlaceId.eq(id))
        .filter(courses::Column::DeletedAt.is_null())
        .exec(&txn)
        .await?;
    places::Entity::update_many()
        .col_expr(places::Column::DeletedAt, Expr::value(now))
        .col_expr(places::Column::UpdatedAt, Expr::value(now))
        .filter(places::Column::Id.eq(id))
        .filter(places::Column::UserId.eq(user_id))
        .filter(places::Column::DeletedAt.is_null())
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(())
}
