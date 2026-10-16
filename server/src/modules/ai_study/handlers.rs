use super::{dto, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::extract::State;
// Keep the AI section validation message when parsing fails.
pub async fn prepare(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(session): AppPath<i64>,
    body: Result<AppJson<dto::Input>, AppError>,
) -> Result<ApiResponse<dto::ImportResource>, AppError> {
    let AppJson(input) = body.map_err(|_| {
        AppError::validation("content", "Nội dung Học cùng AI không hợp lệ hoặc quá lớn.")
    })?;
    Ok(ApiResponse::ok(
        service::prepare(&s.db, u.id, session, input).await?,
    ))
}
