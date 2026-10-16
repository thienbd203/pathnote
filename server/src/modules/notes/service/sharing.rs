use super::*;
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
    let txn = place_service::write_tx(db, user, place.id).await?;
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
    let txn = place_service::write_tx(db, user, place.id).await?;
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
        let literal = crate::pagination::like_contains(term);
        query = query.filter(
            Expr::expr(Func::lower(Expr::col((
                sessions::Entity,
                sessions::Column::Title,
            ))))
            .like(LikeExpr::new(literal).escape('!')),
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
        .select_only()
        .columns([notes::Column::SessionId, notes::Column::SavedAt])
        .into_tuple::<(i64, chrono::DateTime<chrono::FixedOffset>)>()
        .all(db)
        .await?
        .into_iter()
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
        meta: PageMeta::new(input.page, size, total),
    })
}
