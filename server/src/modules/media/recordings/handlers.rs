use super::{
    dto::{PlaybackQuery, PlaybackResource, Progress, RecordingResource},
    service,
};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath, AppQuery},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
pub async fn list(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<Vec<RecordingResource>>, AppError> {
    Ok(ApiResponse::ok(service::list(&s, u.id, id).await?))
}
pub async fn playback(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((id, m)): AppPath<(i64, i64)>,
) -> Result<ApiResponse<PlaybackResource>, AppError> {
    Ok(ApiResponse::ok(service::playback(&s, u.id, id, m).await?))
}
pub async fn progress(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((id, m)): AppPath<(i64, i64)>,
    AppJson(input): AppJson<Progress>,
) -> Result<ApiResponse<Progress>, AppError> {
    Ok(ApiResponse::ok(
        service::progress(&s, u.id, id, m, input).await?,
    ))
}
pub async fn stream(
    State(s): State<AppState>,
    AppPath((id, m)): AppPath<(i64, i64)>,
    AppQuery(query): AppQuery<PlaybackQuery>,
    request: Request,
) -> Result<Response, AppError> {
    let (path, mime) = service::stream(&s, id, m, &query.ticket).await?;
    // Delegate RFC Range/HEAD handling and bounded streaming to tower-http.
    let mut response = tower_http::services::ServeFile::new(path)
        .try_call(request)
        .await
        .map_err(|e| AppError::Internal(format!("Playback storage: {e}")))?
        .into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime).map_err(|_| AppError::Internal("Invalid MIME".into()))?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    Ok(response)
}

pub async fn read_progress(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((id, m)): AppPath<(i64, i64)>,
) -> Result<ApiResponse<Progress>, AppError> {
    Ok(ApiResponse::ok(
        service::read_progress(&s, u.id, id, m).await?,
    ))
}
