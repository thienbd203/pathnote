use super::{dto, service};
use crate::{
    access_token::CurrentUser, pagination::ListQuery, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
pub async fn show(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<dto::NoteResource>, AppError> {
    Ok(ApiResponse::ok(service::get(&s.db, u.id, id).await?))
}
pub async fn save(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<dto::SaveNote>, JsonRejection>,
) -> Result<Response, AppError> {
    service::access(s.db.as_ref(), u.id, id, false).await?;
    if s.config.collaboration_url.is_some() {
        return Err(AppError::BadRequest(
            "Ghi chép đang dùng realtime. Hãy tải lại trang.".into(),
        ));
    }
    let Json(input) =
        body.map_err(|_| AppError::validation("content", "Dữ liệu ghi chép không hợp lệ."))?;
    match service::save(&s.db, u.id, id, input).await? {
        dto::SaveOutcome::Saved(note) => Ok(ApiResponse::ok(note).into_response()),
        dto::SaveOutcome::Conflict(note) => Ok((StatusCode::CONFLICT, Json(serde_json::json!({"message":"Ghi chép đã thay đổi ở một tab khác. Bản đang viết của bạn vẫn được giữ.","data":note}))).into_response()),
    }
}
pub async fn members(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<Vec<dto::Member>>, AppError> {
    Ok(ApiResponse::ok(service::members(&s.db, u.id, id).await?))
}
pub async fn invite(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<dto::Invite>, JsonRejection>,
) -> Result<ApiResponse<dto::Member>, AppError> {
    let Json(input) = body.map_err(|_| AppError::validation("email", "Email không hợp lệ."))?;
    Ok(ApiResponse::created(
        service::invite(&s.db, u.id, id, input.email).await?,
    ))
}
pub async fn revoke(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((id, member)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    service::revoke(&s.db, u.id, id, member).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn shared(
    State(s): State<AppState>,
    u: CurrentUser,
    Query(input): Query<ListQuery>,
) -> Result<Json<crate::pagination::Page<dto::SharedNote>>, AppError> {
    Ok(Json(service::shared(&s.db, u.id, input).await?))
}
pub async fn metadata(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<dto::SharedNote>, AppError> {
    Ok(ApiResponse::ok(service::metadata(&s.db, u.id, id).await?))
}
