use super::{
    dto::{Filters, Resource},
    service,
};
use crate::{access_token::CurrentUser, extract::AppQuery, state::AppState, AppError};
use axum::{extract::State, Json};
pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<AppQuery<Filters>, AppError>,
) -> Result<Json<Resource>, AppError> {
    let AppQuery(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    Ok(Json(service::list(&state.db, user.id, input).await?))
}
