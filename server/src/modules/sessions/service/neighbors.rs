use super::*;
pub async fn neighbors(db: &DatabaseConnection, user: i64, id: i64) -> Result<Neighbors, AppError> {
    let current = model(db, user, id).await?;
    let scope = owned(user).filter(sessions::Column::PlaceId.eq(current.place_id));
    let scope = match current.course_id {
        Some(id) => scope.filter(sessions::Column::CourseId.eq(id)),
        None => scope.filter(sessions::Column::CourseId.is_null()),
    };
    let previous = scope
        .clone()
        .filter(
            Condition::any()
                .add(sessions::Column::StartsAt.lt(current.starts_at))
                .add(
                    Condition::all()
                        .add(sessions::Column::StartsAt.eq(current.starts_at))
                        .add(sessions::Column::Id.lt(id)),
                ),
        )
        .order_by_desc(sessions::Column::StartsAt)
        .order_by_desc(sessions::Column::Id)
        .select_only()
        .column(sessions::Column::Id)
        .limit(1)
        .into_query();
    let next = scope
        .filter(
            Condition::any()
                .add(sessions::Column::StartsAt.gt(current.starts_at))
                .add(
                    Condition::all()
                        .add(sessions::Column::StartsAt.eq(current.starts_at))
                        .add(sessions::Column::Id.gt(id)),
                ),
        )
        .order_by_asc(sessions::Column::StartsAt)
        .order_by_asc(sessions::Column::Id)
        .select_only()
        .column(sessions::Column::Id)
        .limit(1)
        .into_query();
    let previous_ids = Query::select()
        .column((Alias::new("previous"), Alias::new("id")))
        .from_subquery(previous, Alias::new("previous"))
        .to_owned();
    let next_ids = Query::select()
        .column((Alias::new("next"), Alias::new("id")))
        .from_subquery(next, Alias::new("next"))
        .to_owned();
    let mut ids = previous_ids;
    ids.union(UnionType::All, next_ids);
    let rows = owned(user)
        .filter(sessions::Column::Id.in_subquery(ids))
        .select_also(places::Entity)
        .select_also(courses::Entity)
        .all(db)
        .await?;
    let mut neighbors = Neighbors {
        previous: None,
        next: None,
    };
    for (session, place, course) in rows {
        let before = (session.starts_at, session.id) < (current.starts_at, id);
        let resource = SessionResource::new(session, place.ok_or_else(missing)?, course);
        if before {
            neighbors.previous = Some(resource);
        } else {
            neighbors.next = Some(resource);
        }
    }
    Ok(neighbors)
}
