use super::{dto, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::extract::State;
// Preserve the exercise definition error field for malformed request bodies.
fn body<T>(input: Result<AppJson<T>, AppError>) -> Result<T, AppError> {
    input.map(|AppJson(v)| v).map_err(|_| dto::invalid())
}
pub async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(session): AppPath<i64>,
    input: Result<AppJson<dto::Definition>, AppError>,
) -> Result<ApiResponse<dto::CreateResource>, AppError> {
    Ok(ApiResponse::created(
        service::create(&s.db, u.id, session, body(input)?, false).await?,
    ))
}
pub async fn import(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(session): AppPath<i64>,
    input: Result<AppJson<dto::Definition>, AppError>,
) -> Result<ApiResponse<dto::CreateResource>, AppError> {
    Ok(ApiResponse::created(
        service::create(&s.db, u.id, session, body(input)?, true).await?,
    ))
}
pub async fn show(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((session, id)): AppPath<(i64, i64)>,
) -> Result<ApiResponse<dto::ShowResource>, AppError> {
    Ok(ApiResponse::ok(
        service::show(&s.db, u.id, session, id, false).await?,
    ))
}
pub async fn definition(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((session, id)): AppPath<(i64, i64)>,
) -> Result<ApiResponse<dto::ShowResource>, AppError> {
    Ok(ApiResponse::ok(
        service::show(&s.db, u.id, session, id, true).await?,
    ))
}
pub async fn update(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((session, id)): AppPath<(i64, i64)>,
    input: Result<AppJson<dto::Definition>, AppError>,
) -> Result<ApiResponse<dto::IdResource>, AppError> {
    Ok(ApiResponse::ok(
        service::update(&s.db, u.id, session, id, body(input)?).await?,
    ))
}
pub async fn start(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((session, id)): AppPath<(i64, i64)>,
    input: Result<AppJson<dto::Start>, AppError>,
) -> Result<ApiResponse<dto::AttemptResource>, AppError> {
    Ok(ApiResponse::ok(
        service::start(&s.db, u.id, session, id, body(input)?).await?,
    ))
}
pub async fn answer(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((session, id, attempt)): AppPath<(i64, i64, i64)>,
    input: Result<AppJson<dto::Answer>, AppError>,
) -> Result<ApiResponse<dto::AttemptResource>, AppError> {
    Ok(ApiResponse::ok(
        service::answer(&s.db, u.id, session, id, attempt, body(input)?).await?,
    ))
}
