use crate::{
    access_token::CurrentUser, modules::notes::service, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ipa {
    text: String,
    locale: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    texts: Vec<String>,
    locale: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    text: String,
}
fn text(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty()
        || value.chars().count() > 1000
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(AppError::validation(
            "text",
            "Nội dung cần từ 1 đến 1000 ký tự.",
        ));
    }
    Ok(())
}
fn locale(value: &str) -> Result<(), AppError> {
    if !matches!(value, "en-US" | "en-GB") {
        return Err(AppError::validation("locale", "Chọn US hoặc UK."));
    }
    Ok(())
}
pub async fn ipa(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<Ipa>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    service::access(s.db.as_ref(), user.id, id, false).await?;
    let Json(input) = body.map_err(|_| AppError::validation("text", "Nội dung không hợp lệ."))?;
    text(&input.text)?;
    locale(&input.locale)?;
    Ok(ApiResponse::ok(
        s.learning_services
            .ipa(input.text.trim(), &input.locale)
            .await?,
    ))
}
pub async fn batch(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<Batch>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    service::access(s.db.as_ref(), user.id, id, false).await?;
    let Json(input) =
        body.map_err(|_| AppError::validation("texts", "Lô phiên âm không hợp lệ."))?;
    if input.texts.is_empty()
        || input.texts.len() > 50
        || serde_json::to_vec(&input.texts)
            .map_err(|_| AppError::BadRequest("Lô không hợp lệ.".into()))?
            .len()
            > 65536
    {
        return Err(AppError::validation(
            "texts",
            "Mỗi lô từ 1 đến 50 nội dung, tối đa 64 KiB.",
        ));
    }
    locale(&input.locale)?;
    for item in &input.texts {
        text(item)?;
    }
    Ok(ApiResponse::ok(
        s.learning_services
            .batch(&input.texts, &input.locale)
            .await?,
    ))
}
pub async fn translate(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    body: Result<Json<Text>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    service::access(s.db.as_ref(), user.id, id, false).await?;
    let Json(input) = body.map_err(|_| AppError::validation("text", "Nội dung không hợp lệ."))?;
    text(&input.text)?;
    Ok(ApiResponse::ok(
        s.learning_services.translate(input.text.trim()).await?,
    ))
}
