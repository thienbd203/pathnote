use super::owned;
use crate::{
    entities::{courses, places},
    modules::courses::dto::CourseResource,
    pagination::IdsQuery,
    AppError,
};
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter};
pub async fn read_many(
    db: &DatabaseConnection,
    user: i64,
    input: IdsQuery,
) -> Result<Vec<CourseResource>, AppError> {
    let ids = input.validate()?;
    let rows = owned(user)
        .filter(courses::Column::Id.is_in(ids.clone()))
        .select_also(places::Entity)
        .all(db)
        .await?;
    let mut rows = rows
        .into_iter()
        .map(|(row, parent)| {
            Ok((
                row.id,
                CourseResource::new(row, parent.ok_or_else(super::missing)?),
            ))
        })
        .collect::<Result<std::collections::HashMap<_, _>, AppError>>()?;
    Ok(ids.into_iter().filter_map(|id| rows.remove(&id)).collect())
}
