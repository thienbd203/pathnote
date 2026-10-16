use super::{dto, service};
use crate::extract::{AppJson, AppPath, AppQuery};
use crate::{
    access_token::CurrentUser, pagination::ListQuery, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{extract::State, http::StatusCode, Json};
pub async fn show(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<dto::NoteResource>, AppError> {
    Ok(ApiResponse::ok(service::get(&s.db, u.id, id).await?))
}
pub async fn save(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    // Preserve the editor's content-specific validation guidance.
    body: Result<AppJson<dto::SaveNote>, AppError>,
) -> Result<ApiResponse<dto::NoteResource>, AppError> {
    let input = body
        .map(|AppJson(input)| input)
        .map_err(|_| AppError::validation("content", "Dữ liệu ghi chép không hợp lệ."));
    Ok(ApiResponse::ok(
        service::save_rest(
            &s.db,
            user.id,
            id,
            input,
            s.config.collaboration_url.is_some(),
        )
        .await?,
    ))
}
pub async fn members(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<Vec<dto::Member>>, AppError> {
    Ok(ApiResponse::ok(service::members(&s.db, u.id, id).await?))
}
pub async fn invite(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
    // Preserve the invitation form error field.
    body: Result<AppJson<dto::Invite>, AppError>,
) -> Result<ApiResponse<dto::Member>, AppError> {
    let AppJson(input) = body.map_err(|_| AppError::validation("email", "Email không hợp lệ."))?;
    Ok(ApiResponse::created(
        service::invite(&s.db, u.id, id, input.email).await?,
    ))
}
pub async fn revoke(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath((id, member)): AppPath<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    service::revoke(&s.db, u.id, id, member).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn shared(
    State(s): State<AppState>,
    u: CurrentUser,
    AppQuery(input): AppQuery<ListQuery>,
) -> Result<Json<crate::pagination::Page<dto::SharedNote>>, AppError> {
    Ok(Json(service::shared(&s.db, u.id, input).await?))
}
pub async fn metadata(
    State(s): State<AppState>,
    u: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<dto::SharedNote>, AppError> {
    Ok(ApiResponse::ok(service::metadata(&s.db, u.id, id).await?))
}

pub async fn collaboration(
    State(s): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
) -> Result<ApiResponse<dto::CollaborationConfigResource>, AppError> {
    Ok(ApiResponse::ok(
        service::collaboration::config(
            &s.db,
            user.id,
            id,
            &s.config,
            s.learning_services.enabled(),
        )
        .await?,
    ))
}
