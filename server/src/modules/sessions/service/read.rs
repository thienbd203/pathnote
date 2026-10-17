use super::{missing, owned};
use crate::{
    entities::{courses, places, sessions},
    modules::sessions::dto::SessionResource,
    pagination::IdsQuery,
    AppError,
};
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter};

pub async fn read_many(
    db: &DatabaseConnection,
    user: i64,
    input: IdsQuery,
) -> Result<Vec<SessionResource>, AppError> {
    let ids = input.validate()?;
    let rows = owned(user)
        .filter(sessions::Column::Id.is_in(ids.clone()))
        .select_also(places::Entity)
        .select_also(courses::Entity)
        .all(db)
        .await?;
    let mut result = std::collections::HashMap::new();
    for (session, place, course) in rows {
        result.insert(
            session.id,
            SessionResource::new(session, place.ok_or_else(missing)?, course),
        );
    }
    Ok(ids
        .into_iter()
        .filter_map(|id| result.remove(&id))
        .collect())
}
