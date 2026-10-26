mod counts;
mod read;
use super::dto::{self, CoursePage, CourseResource, CreateCourse, UpdateCourse};
use crate::modules::places::service as place_service;
use crate::{
    entities::{courses, places},
    pagination::{ListQuery, PageMeta},
    AppError,
};
pub use read::read_many;
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Select, Set,
};

fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy khóa học.".into())
}
pub(in crate::modules) fn owned(user_id: i64) -> Select<courses::Entity> {
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
    search(db, user_id, Some(place_id), input).await
}
pub async fn search(
    db: &DatabaseConnection,
    user_id: i64,
    place_id: Option<i64>,
    input: ListQuery,
) -> Result<CoursePage, AppError> {
    let size = input.page_size()?;
    let mut query = owned(user_id);
    if let Some(id) = place_id {
        query = query.filter(courses::Column::PlaceId.eq(id));
    }
    if let Some(term) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let literal = crate::pagination::like_contains(term);
        query = query.filter(
            Expr::expr(Func::lower(Expr::col((
                courses::Entity,
                courses::Column::Name,
            ))))
            .like(LikeExpr::new(literal).escape('!')),
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
        meta: PageMeta::new(input.page, size, total),
    })
}
pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    place_id: i64,
    input: CreateCourse,
) -> Result<CourseResource, AppError> {
    place_service::get(db, user_id, place_id).await?;
    dto::validate_dates(input.starts_on, input.ends_on)?;
    let name = dto::validate_name(input.name)?;
    let description = dto::validate_description(input.description)?;
    let txn = place_service::write_tx(db, user_id, place_id).await?;
    let parent = place_service::get(&txn, user_id, place_id).await?;
    let now = chrono::Utc::now().fixed_offset();
    let course = courses::ActiveModel {
        place_id: Set(place_id),
        name: Set(name),
        description: Set(description),
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
    let txn = place_service::write_tx(db, user_id, parent_id).await?;
    let parent = place_service::get(&txn, user_id, parent_id).await?;
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
    model(db, user_id, id).await?;
    crate::modules::bulk::service::remove(
        db,
        user_id,
        crate::modules::bulk::Kind::Course,
        crate::modules::bulk::Input { ids: vec![id] },
    )
    .await
}

use sea_orm::{DbBackend, FromQueryResult, Statement};
// Count blocks in the current note, including realtime saves. Stale extracted cards
// or exercises which were created but never inserted in a note are not counted.
pub async fn library_stats(
    db: &DatabaseConnection,
    user_id: i64,
    id: i64,
) -> Result<super::dto::CourseStats, AppError> {
    model(db, user_id, id).await?;
    // PostgreSQL recursive JSONPath counts require these shared SQL expressions.
    let rows = super::dto::SessionStats::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            r#"SELECT s.id, s.title, s.starts_at,
            {words} AS words,
            {exercises} AS exercises
           FROM sessions s
           JOIN courses c ON c.id = s.course_id AND c.deleted_at IS NULL
           JOIN places p ON p.id = s.place_id AND p.deleted_at IS NULL
           LEFT JOIN notes n ON n.session_id = s.id
           WHERE s.course_id = $1 AND p.user_id = $2 AND s.deleted_at IS NULL
           ORDER BY s.starts_at, s.id"#,
            words = counts::WORDS,
            exercises = counts::EXERCISES
        ),
        [id.into(), user_id.into()],
    ))
    .all(db)
    .await?;
    let latest_past_session = rows
        .iter()
        .rev()
        .find(|row| row.starts_at <= chrono::Utc::now())
        .map(|row| super::dto::LatestSession {
            id: row.id,
            title: row.title.clone(),
            starts_at: row.starts_at.with_timezone(&chrono::Utc),
        });
    Ok(super::dto::CourseStats {
        session_count: rows.len(),
        latest_past_session,
        words: rows.iter().map(|row| row.words).sum(),
        exercises: rows.iter().map(|row| row.exercises).sum(),
        sessions: rows,
    })
}

pub async fn library_summaries(
    db: &DatabaseConnection,
    user: i64,
    query: crate::pagination::IdsQuery,
) -> Result<Vec<dto::CourseSummary>, AppError> {
    let ids = query.validate()?;
    // PostgreSQL JSONPath plus LATERAL aggregation; identifiers are static, values bound.
    let rows = dto::SummaryRow::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
        format!(r#"SELECT c.id, COUNT(s.id)::bigint AS session_count,
          COALESCE(SUM({words}),0)::bigint AS words,
          COALESCE(SUM({exercises}),0)::bigint AS exercises,
          latest.id AS latest_id, latest.title AS latest_title, latest.starts_at AS latest_starts_at
        FROM courses c JOIN places p ON p.id=c.place_id AND p.deleted_at IS NULL
        LEFT JOIN sessions s ON s.course_id=c.id AND s.place_id=p.id AND s.deleted_at IS NULL
        LEFT JOIN notes n ON n.session_id=s.id
        LEFT JOIN LATERAL (SELECT id,title,starts_at FROM sessions WHERE course_id=c.id AND place_id=p.id AND deleted_at IS NULL AND starts_at <= CURRENT_TIMESTAMP ORDER BY starts_at DESC,id DESC LIMIT 1) latest ON true
        WHERE c.id=ANY($1) AND p.user_id=$2 AND c.deleted_at IS NULL
        GROUP BY c.id, latest.id, latest.title, latest.starts_at"#, words=counts::WORDS, exercises=counts::EXERCISES),
        [ids.clone().into(),user.into()])).all(db).await?;
    let mut rows: std::collections::HashMap<_, _> = rows
        .into_iter()
        .map(|row| (row.id, dto::CourseSummary::from(row)))
        .collect();
    Ok(ids.into_iter().filter_map(|id| rows.remove(&id)).collect())
}
