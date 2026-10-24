use super::{dto, optimization as optimizer, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath, AppQuery},
    pagination::Page,
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{extract::State, Json};
use uuid::Uuid;
pub async fn overview(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<AppQuery<dto::Filters>, AppError>,
) -> Result<ApiResponse<dto::Overview>, AppError> {
    let AppQuery(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::overview(&s.db, user.id, input).await?,
    ))
}
pub async fn configure(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<dto::SettingsInput>, AppError>,
) -> Result<ApiResponse<dto::Overview>, AppError> {
    let AppJson(input) =
        input.map_err(|_| AppError::validation("settings", "Cấu hình không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::configure(&s.db, user.id, input).await?,
    ))
}
pub async fn next(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<dto::Filters>, AppError>,
) -> Result<ApiResponse<dto::Next>, AppError> {
    let AppJson(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    Ok(ApiResponse::ok(service::next(&s.db, user.id, input).await?))
}
pub async fn reveal(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> Result<ApiResponse<dto::Reveal>, AppError> {
    Ok(ApiResponse::ok(service::reveal(&s.db, user.id, id).await?))
}
pub async fn show_run(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> Result<ApiResponse<dto::RunSummary>, AppError> {
    Ok(ApiResponse::ok(
        service::show_run(&s.db, user.id, id).await?,
    ))
}
pub async fn finish_run(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> Result<ApiResponse<dto::RunSummary>, AppError> {
    Ok(ApiResponse::ok(
        service::finish_run(&s.db, user.id, id).await?,
    ))
}
pub async fn grade(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
    input: Result<AppJson<dto::Grade>, AppError>,
) -> Result<ApiResponse<dto::GradeResult>, AppError> {
    let AppJson(input) =
        input.map_err(|_| AppError::validation("rating", "Đánh giá không hợp lệ."))?;
    Ok(ApiResponse::ok(
        service::grade(&s.db, user.id, id, input).await?,
    ))
}
pub async fn create_run(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<AppJson<dto::Filters>, AppError>,
) -> Result<ApiResponse<dto::RunSummary>, AppError> {
    let AppJson(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    Ok(ApiResponse::created(
        service::create_run(&s.db, user.id, input).await?,
    ))
}
pub async fn history(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<AppQuery<dto::HistoryFilters>, AppError>,
) -> Result<Json<Page<dto::HistoryEntry>>, AppError> {
    let AppQuery(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc lịch sử không hợp lệ."))?;
    Ok(Json(service::history(&s.db, user.id, input).await?))
}
pub async fn optimization(
    State(s): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<dto::Optimization>, AppError> {
    Ok(ApiResponse::ok(optimizer::status(&s.db, user.id).await?))
}

pub async fn statistics(
    State(s): State<AppState>,
    user: CurrentUser,
    AppQuery(input): AppQuery<dto::StatisticsInput>,
) -> Result<ApiResponse<dto::StudyStatistics>, AppError> {
    Ok(ApiResponse::ok(
        service::statistics(&s.db, user.id, input).await?,
    ))
}
pub async fn record_time(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
    AppJson(input): AppJson<dto::StudyTimeInput>,
) -> Result<ApiResponse<dto::StudyTimeRecorded>, AppError> {
    Ok(ApiResponse::ok(
        service::record_time(&s.db, user.id, id, input).await?,
    ))
}
