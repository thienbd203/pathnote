use super::{
    dto::{Input, Kind},
    service,
};
use crate::{access_token::CurrentUser, extract::AppJson, state::AppState, AppError};
use axum::{extract::State, http::StatusCode};
async fn handler(
    s: AppState,
    u: CurrentUser,
    kind: Kind,
    body: Result<AppJson<Input>, AppError>,
) -> Result<StatusCode, AppError> {
    // Giữ lỗi ids để giao diện gắn thông báo vào danh sách mục được chọn.
    let AppJson(input) =
        body.map_err(|_| AppError::validation("ids", "Danh sách ID không hợp lệ."))?;
    service::remove(&s.db, u.id, kind, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn places(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<AppJson<Input>, AppError>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Place, b).await
}
pub async fn courses(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<AppJson<Input>, AppError>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Course, b).await
}
pub async fn sessions(
    State(s): State<AppState>,
    u: CurrentUser,
    b: Result<AppJson<Input>, AppError>,
) -> Result<StatusCode, AppError> {
    handler(s, u, Kind::Session, b).await
}
