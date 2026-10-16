use super::dto::*;
use crate::{
    entities::{courses, note_collaborators, notes, places, sessions, users},
    modules::places::service as place_service,
    pagination::{ListQuery, Page, PageMeta},
    AppError,
};
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr, Query},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    JoinType, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    Select, Set, TransactionTrait,
};
use serde_json::json;
fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy ghi chép.".into())
}
pub(super) fn active() -> Select<sessions::Entity> {
    sessions::Entity::find()
        .join(JoinType::InnerJoin, sessions::Relation::Place.def())
        .join(JoinType::LeftJoin, sessions::Relation::Course.def())
        .filter(sessions::Column::DeletedAt.is_null())
        .filter(places::Column::DeletedAt.is_null())
        .filter(
            Condition::any()
                .add(sessions::Column::CourseId.is_null())
                .add(courses::Column::DeletedAt.is_null()),
        )
}
fn memberships(user: i64) -> sea_orm::sea_query::SelectStatement {
    Query::select()
        .column(note_collaborators::Column::SessionId)
        .from(note_collaborators::Entity)
        .and_where(Expr::col(note_collaborators::Column::UserId).eq(user))
        .to_owned()
}
pub async fn access<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: i64,
    owner_only: bool,
) -> Result<(sessions::Model, places::Model), AppError> {
    let (session, place) = accessible(user, owner_only)
        .filter(sessions::Column::Id.eq(id))
        .select_also(places::Entity)
        .one(db)
        .await?
        .ok_or_else(missing)?;
    Ok((session, place.ok_or_else(missing)?))
}
pub(in crate::modules) fn accessible(user: i64, owner_only: bool) -> Select<sessions::Entity> {
    let mut condition = Condition::any().add(places::Column::UserId.eq(user));
    if !owner_only {
        condition = condition
            .add(Expr::col((sessions::Entity, sessions::Column::Id)).in_subquery(memberships(user)))
    }
    active().filter(condition)
}
pub async fn metadata(db: &DatabaseConnection, user: i64, id: i64) -> Result<SharedNote, AppError> {
    let (s, _) = access(db, user, id, false).await?;
    Ok(SharedNote {
        id: s.id,
        title: s.title,
        note_saved_at: notes::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|n| n.saved_at),
    })
}
pub(super) async fn note<C: ConnectionTrait>(db: &C, id: i64) -> Result<NoteResource, AppError> {
    Ok(match notes::Entity::find_by_id(id).one(db).await? {
        Some(n) => NoteResource {
            session_id: id,
            content: n.content,
            schema_version: n.schema_version,
            revision: n.revision,
            saved_at: Some(n.saved_at),
        },
        None => NoteResource {
            session_id: id,
            content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
            schema_version: 3,
            revision: 0,
            saved_at: None,
        },
    })
}
pub async fn get(db: &DatabaseConnection, user: i64, id: i64) -> Result<NoteResource, AppError> {
    access(db, user, id, false).await?;
    note(db, id).await
}
pub async fn save(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    input: SaveNote,
) -> Result<SaveOutcome, AppError> {
    let (_, place) = access(db, user, id, false).await?;
    let references = super::document::validate(&input.content, input.schema_version)?;
    let vocabulary = crate::modules::vocabulary::sync::prepare(&input.content)?;
    if !(0..i32::MAX).contains(&input.base_revision) {
        return Err(AppError::validation(
            "base_revision",
            "Phiên bản ghi chép không hợp lệ.",
        ));
    }
    let txn = db.begin().await?;
    // Share/revoke/delete use the same parent lock; recheck access after waiting.
    place_service::lock_for_write(&txn, place.user_id, place.id).await?;
    access(&txn, user, id, false).await?;
    validate_references(&txn, id, &references).await?;
    let current = note(&txn, id).await?;
    if input.schema_version < current.schema_version {
        return Err(AppError::validation(
            "schema_version",
            "Editor đã cũ. Hãy tải lại trang.",
        ));
    }
    if current.revision != input.base_revision {
        return Ok(SaveOutcome::Conflict(current));
    }
    if notes::Entity::find_by_id(id)
        .one(&txn)
        .await?
        .is_some_and(|n| n.yjs_state.is_some())
    {
        return Err(AppError::BadRequest(
            "Ghi chép này dùng realtime. Hãy tải lại trang để tiếp tục.".into(),
        ));
    }
    let next = notes::ActiveModel {
        session_id: Set(id),
        content: Set(input.content),
        yjs_state: Set(None),
        schema_version: Set(input.schema_version),
        revision: Set(current.revision + 1),
        saved_at: Set(chrono::Utc::now().fixed_offset()),
    };
    if current.revision == 0 && current.saved_at.is_none() {
        next.insert(&txn).await?;
    } else {
        next.update(&txn).await?;
    }
    sessions::Entity::update_many()
        .col_expr(
            sessions::Column::UpdatedAt,
            Expr::value(chrono::Utc::now().fixed_offset()),
        )
        .filter(sessions::Column::Id.eq(id))
        .exec(&txn)
        .await?;
    let result = note(&txn, id).await?;
    crate::modules::ai_study::service::attach(&txn, id, &references).await?;
    crate::modules::vocabulary::sync::sync(&txn, id, vocabulary).await?;
    txn.commit().await?;
    Ok(SaveOutcome::Saved(result))
}
fn member(u: users::Model) -> Member {
    Member {
        id: u.id,
        name: u.name,
        email: u.email,
    }
}
pub async fn members(db: &DatabaseConnection, user: i64, id: i64) -> Result<Vec<Member>, AppError> {
    let (session, _) = access(db, user, id, true).await?;
    Ok(session
        .find_related(users::Entity)
        .order_by_asc(users::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .map(member)
        .collect())
}
pub async fn invite(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    email: String,
) -> Result<Member, AppError> {
    let (_, place) = access(db, user, id, true).await?;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user, place.id).await?;
    access(&txn, user, id, true).await?;
    let email = email.trim().to_lowercase();
    if email.len() > 255 || !email.contains('@') {
        return Err(AppError::validation("email", "Email không hợp lệ."));
    }
    let u = users::Entity::find()
        .filter(Expr::expr(Func::lower(Expr::col(users::Column::Email))).eq(email))
        .filter(users::Column::GoogleId.is_not_null())
        .one(&txn)
        .await?
        .ok_or_else(|| {
            AppError::validation(
                "email",
                "Người được mời cần đăng nhập Pathnote bằng Google trước.",
            )
        })?;
    if u.id == user {
        return Err(AppError::validation(
            "email",
            "Bạn đã là chủ sở hữu ghi chép này.",
        ));
    }
    note_collaborators::Entity::insert(note_collaborators::ActiveModel {
        session_id: Set(id),
        user_id: Set(u.id),
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::columns([
            note_collaborators::Column::SessionId,
            note_collaborators::Column::UserId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .do_nothing()
    .exec(&txn)
    .await?;
    txn.commit().await?;
    Ok(member(u))
}
pub async fn revoke(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    member: i64,
) -> Result<(), AppError> {
    let (_, place) = access(db, user, id, true).await?;
    let txn = db.begin().await?;
    place_service::lock_for_write(&txn, user, place.id).await?;
    access(&txn, user, id, true).await?;
    note_collaborators::Entity::delete_by_id((id, member))
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(())
}
pub async fn shared(
    db: &DatabaseConnection,
    user: i64,
    input: ListQuery,
) -> Result<Page<SharedNote>, AppError> {
    let size = input.page_size()?;
    let mut query = active().filter(sessions::Column::Id.in_subquery(memberships(user)));
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
    let pages = query
        .order_by_desc(sessions::Column::UpdatedAt)
        .order_by_asc(sessions::Column::Id)
        .paginate(db, size);
    let total = pages.num_items().await?;
    // A single notes query for the page keeps metadata loading bounded.
    let rows = pages.fetch_page(input.page - 1).await?;
    let saved = notes::Entity::find()
        .filter(notes::Column::SessionId.is_in(rows.iter().map(|s| s.id)))
        .all(db)
        .await?
        .into_iter()
        .map(|n| (n.session_id, n.saved_at))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(Page {
        data: rows
            .into_iter()
            .map(|s| SharedNote {
                id: s.id,
                title: s.title,
                note_saved_at: saved.get(&s.id).copied(),
            })
            .collect(),
        meta: PageMeta {
            current_page: input.page,
            last_page: total.div_ceil(size).max(1),
            per_page: size,
            total,
        },
    })
}

pub(super) async fn validate_references<C: ConnectionTrait>(
    db: &C,
    id: i64,
    references: &[super::document::Reference],
) -> Result<(), AppError> {
    use super::document::Reference;
    let media_refs: Vec<_> = references
        .iter()
        .filter_map(|r| match r {
            Reference::Media { id, kind } => Some((*id, *kind)),
            _ => None,
        })
        .collect();
    if !media_refs.is_empty() {
        let media = crate::entities::media::Entity::find()
            .filter(crate::entities::media::Column::SessionId.eq(id))
            .filter(crate::entities::media::Column::Id.is_in(media_refs.iter().map(|r| r.0)))
            .all(db)
            .await?;
        if !media_refs
            .iter()
            .all(|r| media.iter().any(|m| m.id == r.0 && m.kind == r.1))
        {
            return Err(AppError::validation(
                "content",
                "Ảnh hoặc tệp không thuộc buổi học này.",
            ));
        }
    }
    let exercises: Vec<_> = references
        .iter()
        .filter_map(|r| match r {
            Reference::Exercise(id) => Some(*id),
            _ => None,
        })
        .collect();
    crate::modules::ai_study::service::references(db, id, references).await?;
    crate::modules::exercises::service::references(db, id, &exercises).await
}
