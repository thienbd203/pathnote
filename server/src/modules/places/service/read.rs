use super::owned;
use crate::{
    entities::places, modules::places::dto::PlaceResource, pagination::IdsQuery, AppError,
};
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter};
pub async fn read_many(
    db: &DatabaseConnection,
    user: i64,
    input: IdsQuery,
) -> Result<Vec<PlaceResource>, AppError> {
    let ids = input.validate()?;
    let rows = owned(user)
        .filter(places::Column::Id.is_in(ids.clone()))
        .all(db)
        .await?;
    let mut rows: std::collections::HashMap<_, _> = rows
        .into_iter()
        .map(|row| (row.id, PlaceResource::from(row)))
        .collect();
    Ok(ids.into_iter().filter_map(|id| rows.remove(&id)).collect())
}
