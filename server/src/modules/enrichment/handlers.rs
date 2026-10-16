use super::{dto::*, service};
use crate::{
    access_token::CurrentUser,
    extract::{AppJson, AppPath},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::extract::State;
pub async fn ipa(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    body: Result<AppJson<Ipa>, AppError>,
) -> Result<ApiResponse<IpaResource>, AppError> {
    // Preserve validation guidance after the service checks note access.
    let input = body
        .map(|AppJson(value)| value)
        .map_err(|_| AppError::validation("text", "Nội dung không hợp lệ."));
    Ok(ApiResponse::ok(
        service::ipa(&s.db, user.id, id, &s.learning_services, input).await?,
    ))
}
pub async fn batch(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    body: Result<AppJson<Batch>, AppError>,
) -> Result<ApiResponse<BatchResource>, AppError> {
    // Preserve the existing batch-specific error field and message.
    let input = body
        .map(|AppJson(value)| value)
        .map_err(|_| AppError::validation("texts", "Lô phiên âm không hợp lệ."));
    Ok(ApiResponse::ok(
        service::batch(&s.db, user.id, id, &s.learning_services, input).await?,
    ))
}
pub async fn translate(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    body: Result<AppJson<Text>, AppError>,
) -> Result<ApiResponse<TranslationResource>, AppError> {
    // Preserve validation guidance after the service checks note access.
    let input = body
        .map(|AppJson(value)| value)
        .map_err(|_| AppError::validation("text", "Nội dung không hợp lệ."));
    Ok(ApiResponse::ok(
        service::translate(&s.db, user.id, id, &s.learning_services, input).await?,
    ))
}
