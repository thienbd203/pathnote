use super::dto::{self, CoursePage, CourseResource, CreateCourse, UpdateCourse};
use crate::modules::places::service as place_service;
use crate::{
    entities::{courses, places},
    pagination::{ListQuery, PageMeta},
    AppError,
};
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Select, Set,
    TransactionTrait,
};

fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy khóa học.".into())
}
fn owned(user_id: i64) -> Select<courses::Entity> {
    courses::Entity::find()
        .join(JoinType::InnerJoin, courses::Relation::Place.def())
        .filter(places::Column::UserId.eq(user_id))
        .filter(places::Column::DeletedAt.is_null())
        .filter(courses::Column::DeletedAt.is_null())
}
pub async fn model<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
) -> Result<courses::Model, AppError> {
    owned(user_id)
        .filter(courses::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(missing)
}
pub async fn get<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
) -> Result<CourseResource, AppError> {
    let (course, parent) = owned(user_id)
        .filter(courses::Column::Id.eq(id))
        .select_also(places::Entity)
        .one(db)
        .await?
        .ok_or_else(missing)?;
    Ok(CourseResource::new(course, parent.ok_or_else(missing)?))
}
pub async fn list(
    db: &DatabaseConnection,
    user_id: i64,
    place_id: i64,
    input: ListQuery,
) -> Result<CoursePage, AppError> {
    place_service::get(db, user_id, place_id).await?;
    let size = input.page_size()?;
    let mut query = owned(user_id).filter(courses::Column::PlaceId.eq(place_id));
    if let Some(term) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let literal = term
            .to_lowercase()
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        query = query.filter(
            Expr::expr(Func::lower(Expr::col((
                courses::Entity,
                courses::Column::Name,
            ))))
            .like(LikeExpr::new(format!("%{literal}%")).escape('!')),
        );
    }
    // The parent metadata comes from the same JOIN, not one query per course.
    let paginator = query
        .order_by_asc(courses::Column::Id)
        .select_also(places::Entity)
        .paginate(db, size);
    let total = paginator.num_items().await?;
    let data = paginator
        .fetch_page(input.page - 1)
        .await?
        .into_iter()
        .map(|(course, parent)| Ok(CourseResource::new(course, parent.ok_or_else(missing)?)))
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(CoursePage {
        data,
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
    place_id: i64,
    input: CreateCourse,
) -> Result<CourseResource, AppError> {
    let txn = db.begin().await?;
    let parent = place_service::lock_for_write(&txn, user_id, place_id).await?;
    dto::validate_dates(input.starts_on, input.ends_on)?;
    let now = chrono::Utc::now().fixed_offset();
    let course = courses::ActiveModel {
        place_id: Set(place_id),
        name: Set(dto::validate_name(input.name)?),
        description: Set(dto::validate_description(input.description)?),
        starts_on: Set(input.starts_on),
        ends_on: Set(input.ends_on),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(CourseResource::new(course, parent))
}
pub async fn update(
    db: &DatabaseConnection,
    user_id: i64,
    id: i64,
    input: UpdateCourse,
) -> Result<CourseResource, AppError> {
    let parent_id = model(db, user_id, id).await?.place_id;
    let txn = db.begin().await?;
    let parent = place_service::lock_for_write(&txn, user_id, parent_id).await?;
    let current = model(&txn, user_id, id).await?;
    let start = match input.starts_on {
        Some(Some(v)) => v,
        Some(None) => {
            return Err(AppError::validation(
                "starts_on",
                "Ngày bắt đầu không được null.",
            ))
        }
        None => current.starts_on,
    };
    let end = input.ends_on.unwrap_or(current.ends_on);
    dto::validate_dates(start, end)?;
    let mut changes: courses::ActiveModel = current.into();
    if let Some(name) = input.name {
        changes.name = Set(dto::validate_name(name.ok_or_else(|| {
            AppError::validation("name", "Tên khóa học không được null.")
        })?)?);
    }
    if let Some(description) = input.description {
        changes.description = Set(dto::validate_description(description)?);
    }
    changes.starts_on = Set(start);
    changes.ends_on = Set(end);
    changes.updated_at = Set(chrono::Utc::now().fixed_offset());
    let course = changes.update(&txn).await?;
    txn.commit().await?;
    Ok(CourseResource::new(course, parent))
}
pub async fn delete(db: &DatabaseConnection, user_id: i64, id: i64) -> Result<(), AppError> {
    let parent_id = model(db, user_id, id).await?.place_id;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user_id, parent_id).await?;
    let current = model(&txn, user_id, id).await?;
    let now = chrono::Utc::now().fixed_offset();
    let mut changes: courses::ActiveModel = current.into();
    changes.deleted_at = Set(Some(now));
    changes.updated_at = Set(now);
    use crate::entities::sessions;
    sessions::Entity::update_many()
        .col_expr(sessions::Column::DeletedAt, Expr::value(now))
        .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
        .filter(sessions::Column::CourseId.eq(id))
        .filter(sessions::Column::DeletedAt.is_null())
        .exec(&txn)
        .await?;
    changes.update(&txn).await?;
    txn.commit().await?;
    Ok(())
}
