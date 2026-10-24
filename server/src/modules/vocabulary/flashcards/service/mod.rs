mod cards;
mod log;
mod review;
mod statistics;
pub(super) use statistics::{record_time, statistics};
pub(super) mod runs;
use super::{dto, queue};
use crate::AppError;
pub(super) use cards::{configure, next};
pub(super) use log::history;
pub(super) use review::{grade, reveal};
pub(super) use runs::{create as create_run, end as finish_run, show as show_run};
use sea_orm::DatabaseConnection;

pub async fn overview(
    db: &DatabaseConnection,
    user: i64,
    mut filters: dto::Filters,
) -> Result<dto::Overview, AppError> {
    filters.validate()?;
    queue::authorize(db, user, &filters).await?;
    queue::overview(db, user, &filters).await
}
