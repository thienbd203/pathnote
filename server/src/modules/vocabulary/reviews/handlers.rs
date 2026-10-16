use super::{dto, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath, AppQuery},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::extract::State;
pub async fn start(
    State(s): State<AppState>,
    user: CurrentUser,
    body: Result<AppJson<dto::Create>, AppError>,
) -> Result<ApiResponse<dto::ReviewResource>, AppError> {
    let AppJson(input) = body.map_err(|_| AppError::validation("mode", "Lượt ôn không hợp lệ."))?;
    Ok(ApiResponse::created(
        service::create(&s.db, user.id, input).await?,
    ))
}
pub async fn show(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<dto::ReviewResource>, AppError> {
    Ok(ApiResponse::ok(service::show(&s.db, user.id, id).await?))
}
pub async fn submit(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    body: Result<AppJson<dto::AnswerInput>, AppError>,
) -> Result<ApiResponse<dto::AnswerResource>, AppError> {
    let AppJson(input) =
        body.map_err(|_| AppError::validation("answer", "Đáp án không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::answer(&s.db, user.id, id, input).await?,
    ))
}
pub async fn pronunciation(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    query: Result<AppQuery<dto::Position>, AppError>,
) -> Result<ApiResponse<dto::Pronunciation>, AppError> {
    let AppQuery(input) =
        query.map_err(|_| AppError::validation("position", "Vị trí không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::pronunciation(&s.db, user.id, id, input).await?,
    ))
}
