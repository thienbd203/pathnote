use super::dto::*;
use crate::{
    entities::{
        sessions, users, vocabulary_cards as cards, word_group_members as members,
        word_groups as groups,
    },
    pagination::{Page, PageMeta},
    AppError,
};
use sea_orm::{
    sea_query::LockType, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::collections::{HashMap, HashSet};
fn missing() -> AppError {
    AppError::NotFound("Không tìm thấy nhóm từ.".into())
}
fn owned(user: i64) -> sea_orm::Select<groups::Entity> {
    groups::Entity::find().filter(groups::Column::UserId.eq(user))
}
async fn model<C: ConnectionTrait>(db: &C, user: i64, id: i64) -> Result<groups::Model, AppError> {
    owned(user)
        .filter(groups::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(missing)
}
fn revision(current: i32, expected: Option<i32>) -> Result<(), AppError> {
    if Some(current) != expected {
        Err(AppError::Conflict(
            "Nhóm đã thay đổi ở cửa sổ khác. Hãy tải lại nhóm trước khi lưu.".into(),
        ))
    } else {
        Ok(())
    }
}
async fn resources<C: ConnectionTrait>(
    db: &C,
    user: i64,
    rows: Vec<cards::Model>,
) -> Result<HashMap<i64, CardResource>, AppError> {
    if rows.is_empty() {
        return Ok(HashMap::new());
    }
    let sources: HashMap<_, _> = crate::modules::notes::service::accessible(user, false)
        .filter(sessions::Column::Id.is_in(rows.iter().map(|c| c.session_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|s| (s.id, s.title))
        .collect();
    let mut result = HashMap::new();
    for c in rows {
        let Some(title) = sources.get(&c.session_id) else {
            continue;
        };
        let ipa = super::super::pronunciation::flashcard_ipa(&c);
        let examples = serde_json::from_value(super::super::pronunciation::flashcard_examples(&c))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        result.insert(
            c.id,
            CardResource {
                id: c.id,
                word: c.word,
                ipa,
                locale: "en-US".into(),
                part_of_speech: c.part_of_speech,
                meaning: c.meaning,
                examples,
                source: SourceResource {
                    id: c.session_id,
                    title: title.clone(),
                },
            },
        );
    }
    Ok(result)
}
pub async fn list(db: &DatabaseConnection, user: i64) -> Result<Vec<SummaryResource>, AppError> {
    let groups = owned(user)
        .order_by_desc(groups::Column::UpdatedAt)
        .order_by_desc(groups::Column::Id)
        .all(db)
        .await?;
    let memberships = members::Entity::find()
        .filter(members::Column::GroupId.is_in(groups.iter().map(|g| g.id)))
        .all(db)
        .await?;
    let mut counts = HashMap::new();
    for m in memberships {
        *counts.entry(m.group_id).or_insert(0) += 1;
    }
    Ok(groups
        .into_iter()
        .map(|g| SummaryResource {
            id: g.id,
            name: g.name,
            count: counts.get(&g.id).copied().unwrap_or(0),
            revision: g.revision,
            updated_at: g.updated_at,
        })
        .collect())
}
pub async fn get<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: i64,
) -> Result<GroupResource, AppError> {
    let group = model(db, user, id).await?;
    let membership = members::Entity::find()
        .filter(members::Column::GroupId.eq(id))
        .order_by_asc(members::Column::Position)
        .all(db)
        .await?;
    let rows = super::super::library::scope(user, None)
        .filter(cards::Column::Id.is_in(membership.iter().map(|m| m.card_id)))
        .all(db)
        .await?;
    let mut content = resources(db, user, rows).await?;
    Ok(GroupResource {
        id,
        name: group.name,
        note: group.note,
        revision: group.revision,
        updated_at: group.updated_at,
        members: membership
            .into_iter()
            .map(|m| MemberResource {
                card_id: m.card_id,
                card: content.remove(&m.card_id),
            })
            .collect(),
    })
}
pub async fn search(
    db: &DatabaseConnection,
    user: i64,
    input: Search,
) -> Result<Page<CardResource>, AppError> {
    let q = input.q.trim();
    if q.is_empty() || q.chars().count() > 200 || input.page == 0 || input.page > 100000 {
        return Err(AppError::validation(
            "q",
            "Nhập từ hoặc nghĩa, tối đa 200 ký tự.",
        ));
    }
    let query = super::super::library::filtered(
        super::super::library::scope(user, None),
        None,
        Some(q.into()),
    );
    let total = query.clone().count(db).await?;
    let rows = query
        .order_by_asc(cards::Column::Word)
        .order_by_asc(cards::Column::Id)
        .offset((input.page - 1) * 20)
        .limit(20)
        .all(db)
        .await?;
    let ids: Vec<_> = rows.iter().map(|c| c.id).collect();
    let mut values = resources(db, user, rows).await?;
    Ok(Page {
        data: ids
            .into_iter()
            .filter_map(|id| values.remove(&id))
            .collect(),
        meta: PageMeta::new(input.page, 20, total),
    })
}
pub async fn save(
    db: &DatabaseConnection,
    user: i64,
    id: Option<i64>,
    mut input: SaveGroup,
) -> Result<GroupResource, AppError> {
    input.validate()?;
    let tx = db.begin().await?;
    // Serialize this user's group writes, including the bounded group count.
    users::Entity::find_by_id(user)
        .lock(LockType::Update)
        .one(&tx)
        .await?
        .ok_or_else(missing)?;
    let current = if let Some(id) = id {
        let g = model(&tx, user, id).await?;
        revision(g.revision, input.revision)?;
        Some(g)
    } else {
        if input.revision.is_some() {
            return Err(AppError::validation(
                "revision",
                "Nhóm mới không có phiên bản.",
            ));
        }
        if owned(user).count(&tx).await? >= 200 {
            return Err(AppError::validation(
                "name",
                "Bạn có thể lưu tối đa 200 nhóm.",
            ));
        }
        None
    };
    let previous: HashSet<i64> = if let Some(g) = &current {
        members::Entity::find()
            .filter(members::Column::GroupId.eq(g.id))
            .all(&tx)
            .await?
            .into_iter()
            .map(|m| m.card_id)
            .collect()
    } else {
        HashSet::new()
    };
    let added: Vec<_> = input
        .card_ids
        .iter()
        .copied()
        .filter(|id| !previous.contains(id))
        .collect();
    if !added.is_empty()
        && super::super::library::scope(user, None)
            .filter(cards::Column::Id.is_in(added.clone()))
            .count(&tx)
            .await?
            != added.len() as u64
    {
        return Err(AppError::validation(
            "card_ids",
            "Có từ đã bị xóa hoặc bạn không còn quyền xem. Hãy tìm lại từ.",
        ));
    }
    let now = chrono::Utc::now().fixed_offset();
    let group = if let Some(g) = current {
        let next = g
            .revision
            .checked_add(1)
            .ok_or_else(|| AppError::Conflict("Nhóm đã vượt giới hạn phiên bản.".into()))?;
        let mut active: groups::ActiveModel = g.into();
        active.name = Set(input.name);
        active.note = Set(input.note);
        active.revision = Set(next);
        active.updated_at = Set(now);
        active.update(&tx).await?
    } else {
        groups::ActiveModel {
            user_id: Set(user),
            name: Set(input.name),
            note: Set(input.note),
            revision: Set(1),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&tx)
        .await?
    };
    members::Entity::delete_many()
        .filter(members::Column::GroupId.eq(group.id))
        .exec(&tx)
        .await?;
    if !input.card_ids.is_empty() {
        members::Entity::insert_many(input.card_ids.into_iter().enumerate().map(
            |(position, card_id)| members::ActiveModel {
                group_id: Set(group.id),
                card_id: Set(card_id),
                position: Set(position as i32),
            },
        ))
        .exec(&tx)
        .await?;
    }
    // Reads always recheck source access; inactive/revoked members remain removable placeholders.
    let result = get(&tx, user, group.id).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn delete(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    expected: i32,
) -> Result<(), AppError> {
    let tx = db.begin().await?;
    users::Entity::find_by_id(user)
        .lock(LockType::Update)
        .one(&tx)
        .await?
        .ok_or_else(missing)?;
    let g = model(&tx, user, id).await?;
    revision(g.revision, Some(expected))?;
    groups::Entity::delete_by_id(g.id).exec(&tx).await?;
    tx.commit().await?;
    Ok(())
}
