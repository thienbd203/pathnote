use super::{
    dto::{Kind, MediaResource},
    service,
};
use crate::{
    access_token::CurrentUser, modules::notes::service as notes, response::ApiResponse,
    state::AppState, AppError,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
async fn upload(
    s: AppState,
    u: CurrentUser,
    id: i64,
    kind: Kind,
    mut body: Multipart,
) -> Result<ApiResponse<MediaResource>, AppError> {
    let access = notes::access(s.db.as_ref(), u.id, id, false).await?;
    let mut field = body
        .next_field()
        .await
        .map_err(|_| AppError::validation(kind.field(), "Upload không hợp lệ hoặc vượt giới hạn."))?
        .ok_or_else(|| AppError::validation(kind.field(), "Bạn hãy chọn file."))?;
    if field.name() != Some(kind.field()) {
        return Err(AppError::validation(
            kind.field(),
            "Trường upload không hợp lệ.",
        ));
    }
    let name = field
        .file_name()
        .ok_or_else(|| AppError::validation(kind.field(), "Bạn hãy chọn file."))?
        .to_owned();
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(|_| {
        AppError::validation(kind.field(), "Upload không hợp lệ hoặc vượt giới hạn.")
    })? {
        if bytes.len() + chunk.len() > kind.limit() {
            return Err(AppError::validation(
                kind.field(),
                "File vượt giới hạn dung lượng.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    drop(field); // Release multer's field before reading the next part.
    if body
        .next_field()
        .await
        .map_err(|_| AppError::validation(kind.field(), "Upload không hợp lệ."))?
        .is_some()
    {
        return Err(AppError::validation(
            kind.field(),
            "Chỉ upload một file mỗi lần.",
        ));
    }
    Ok(ApiResponse::created(
        service::create(&s, u.id, kind, name, bytes, access).await?,
    ))
}
pub async fn image_upload(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
    body: Multipart,
) -> Result<ApiResponse<MediaResource>, AppError> {
    upload(s, u, id, Kind::Image, body).await
}
pub async fn file_upload(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
    body: Multipart,
) -> Result<ApiResponse<MediaResource>, AppError> {
    upload(s, u, id, Kind::File, body).await
}
pub async fn info(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((id, m)): Path<(i64, i64)>,
) -> Result<ApiResponse<MediaResource>, AppError> {
    Ok(ApiResponse::ok(
        service::get(&s, u.id, id, Kind::File, m).await?.into(),
    ))
}
async fn content(
    s: AppState,
    u: CurrentUser,
    id: i64,
    m: i64,
    kind: Kind,
) -> Result<Response, AppError> {
    let model = service::get(&s, u.id, id, kind, m).await?;
    let bytes = service::bytes(&s, &model).await?;
    let mut r = Body::from(bytes).into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&model.mime_type)
            .map_err(|_| AppError::Internal("Invalid stored MIME".into()))?,
    );
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    r.headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Authorization"));
    r.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    let disposition = if kind == Kind::Image {
        "inline".to_owned()
    } else {
        format!(
            "attachment; filename=\"document\"; filename*=UTF-8''{}",
            percent_encoding::utf8_percent_encode(&model.name, percent_encoding::NON_ALPHANUMERIC)
        )
    };
    r.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|_| AppError::Internal("Invalid filename".into()))?,
    );
    Ok(r)
}
pub async fn image(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((id, m)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    content(s, u, id, m, Kind::Image).await
}
pub async fn download(
    State(s): State<AppState>,
    u: CurrentUser,
    Path((id, m)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    content(s, u, id, m, Kind::File).await
}
