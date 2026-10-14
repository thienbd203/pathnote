use super::dto::{self, CreateSession, SessionPage, SessionResource, UpdateSession};
use crate::{
    entities::{
        courses, places,
        sessions::{self, SessionStatus},
    },
    modules::{courses::service as course_service, places::service as place_service},
    pagination::{ListQuery, PageMeta},
    AppError,
};
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Select, Set,
    TransactionTrait,
};
#[derive(Clone, Copy)]
pub enum Parent {
    Place(i64),
    Course(i64),
}
fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy buổi học.".into())
}
pub(in crate::modules) fn owned(user: i64) -> Select<sessions::Entity> {
    sessions::Entity::find()
        .join(JoinType::InnerJoin, sessions::Relation::Place.def())
        .join(JoinType::LeftJoin, sessions::Relation::Course.def())
        .filter(places::Column::UserId.eq(user))
        .filter(places::Column::DeletedAt.is_null())
        .filter(sessions::Column::DeletedAt.is_null())
        .filter(
            Condition::any()
                .add(sessions::Column::CourseId.is_null())
                .add(courses::Column::DeletedAt.is_null()),
        )
}
async fn parent<C: ConnectionTrait>(
    db: &C,
    user: i64,
    parent: Parent,
) -> Result<(i64, Option<i64>), AppError> {
    match parent {
        Parent::Place(id) => {
            place_service::get(db, user, id).await?;
            Ok((id, None))
        }
        Parent::Course(id) => {
            let course = course_service::model(db, user, id).await?;
            Ok((course.place_id, Some(id)))
        }
    }
}
async fn model<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: i64,
) -> Result<sessions::Model, AppError> {
    owned(user)
        .filter(sessions::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(missing)
}
pub async fn get<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: i64,
) -> Result<SessionResource, AppError> {
    let (session, place, course) = owned(user)
        .filter(sessions::Column::Id.eq(id))
        .select_also(places::Entity)
        .select_also(courses::Entity)
        .one(db)
        .await?
        .ok_or_else(missing)?;
    Ok(SessionResource::new(
        session,
        place.ok_or_else(missing)?,
        course,
    ))
}
pub async fn list(
    db: &DatabaseConnection,
    user: i64,
    scope: Parent,
    input: ListQuery,
) -> Result<SessionPage, AppError> {
    let (place_id, course_id) = parent(db, user, scope).await?;
    input.page_size()?;
    let mut query = owned(user).filter(sessions::Column::PlaceId.eq(place_id));
    query = match course_id {
        Some(id) => query.filter(sessions::Column::CourseId.eq(id)),
        None => query.filter(sessions::Column::CourseId.is_null()),
    };
    if let Some(term) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let literal = term
            .to_lowercase()
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        query = query.filter(
            Expr::expr(Func::lower(Expr::col((
                sessions::Entity,
                sessions::Column::Title,
            ))))
            .like(LikeExpr::new(format!("%{literal}%")).escape('!')),
        );
    }
    page(db, query, input).await
}
pub(super) async fn page(
    db: &DatabaseConnection,
    query: Select<sessions::Entity>,
    input: ListQuery,
) -> Result<SessionPage, AppError> {
    let size = input.page_size()?;
    let paginator = query
        .order_by_asc(sessions::Column::StartsAt)
        .order_by_asc(sessions::Column::Id)
        .select_also(places::Entity)
        .select_also(courses::Entity)
        .paginate(db, size);
    let total = paginator.num_items().await?;
    let data = paginator
        .fetch_page(input.page - 1)
        .await?
        .into_iter()
        .map(|(session, place, course)| {
            Ok(SessionResource::new(
                session,
                place.ok_or_else(missing)?,
                course,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(SessionPage {
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
    user: i64,
    scope: Parent,
    input: CreateSession,
) -> Result<SessionResource, AppError> {
    let (place_id, _) = parent(db, user, scope).await?;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user, place_id).await?;
    let (_, course_id) = parent(&txn, user, scope).await?;
    dto::time(
        input.starts_at,
        input.duration_minutes,
        dto::timezone(&input.timezone)?,
    )?;
    let now = chrono::Utc::now().fixed_offset();
    let session = sessions::ActiveModel {
        place_id: Set(place_id),
        course_id: Set(course_id),
        title: Set(dto::title(input.title)?),
        starts_at: Set(input.starts_at.fixed_offset()),
        duration_minutes: Set(input.duration_minutes as i16),
        status: Set(input.status.unwrap_or(SessionStatus::Scheduled)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    let result = get(&txn, user, session.id).await?;
    txn.commit().await?;
    Ok(result)
}
pub async fn update(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    input: UpdateSession,
) -> Result<SessionResource, AppError> {
    let place_id = model(db, user, id).await?.place_id;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user, place_id).await?;
    let current = model(&txn, user, id).await?;
    let changes_time = input.starts_at.is_some() || input.duration_minutes.is_some();
    let start = match input.starts_at {
        Some(Some(v)) => v,
        Some(None) => {
            return Err(AppError::validation(
                "starts_at",
                "Giờ bắt đầu không được null.",
            ))
        }
        None => current.starts_at.with_timezone(&chrono::Utc),
    };
    let duration = match input.duration_minutes {
        Some(Some(v)) => v,
        Some(None) => {
            return Err(AppError::validation(
                "duration_minutes",
                "Thời lượng không được null.",
            ))
        }
        None => current.duration_minutes as i32,
    };
    let timezone = input.timezone.as_deref().map(dto::timezone).transpose()?;
    if changes_time {
        dto::time(
            start,
            duration,
            timezone.ok_or_else(|| {
                AppError::validation("timezone", "Cần múi giờ khi đổi thời gian hoặc thời lượng.")
            })?,
        )?;
    }
    let mut changes: sessions::ActiveModel = current.into();
    if let Some(title) = input.title {
        changes.title = Set(dto::title(title.ok_or_else(|| {
            AppError::validation("title", "Tên buổi học không được null.")
        })?)?);
    }
    if let Some(status) = input.status {
        changes.status =
            Set(status
                .ok_or_else(|| AppError::validation("status", "Trạng thái không được null."))?);
    }
    changes.starts_at = Set(start.fixed_offset());
    changes.duration_minutes = Set(duration as i16);
    changes.updated_at = Set(chrono::Utc::now().fixed_offset());
    changes.update(&txn).await?;
    let result = get(&txn, user, id).await?;
    txn.commit().await?;
    Ok(result)
}
pub async fn delete(db: &DatabaseConnection, user: i64, id: i64) -> Result<(), AppError> {
    let place_id = model(db, user, id).await?.place_id;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user, place_id).await?;
    let current = model(&txn, user, id).await?;
    let now = chrono::Utc::now().fixed_offset();
    let mut changes: sessions::ActiveModel = current.into();
    changes.deleted_at = Set(Some(now));
    changes.updated_at = Set(now);
    changes.update(&txn).await?;
    txn.commit().await?;
    Ok(())
}
