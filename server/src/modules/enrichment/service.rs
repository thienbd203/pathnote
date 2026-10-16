use super::{client::LearningServices, dto::*};
use crate::{modules::notes::service as notes, AppError};
use sea_orm::DatabaseConnection;
pub async fn ipa(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    services: &LearningServices,
    input: Result<Ipa, AppError>,
) -> Result<IpaResource, AppError> {
    notes::access(db, user, id, false).await?;
    let input = input?;
    validate_text(&input.text)?;
    validate_locale(&input.locale)?;
    services.ipa(input.text.trim(), &input.locale).await
}
pub async fn batch(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    services: &LearningServices,
    input: Result<Batch, AppError>,
) -> Result<BatchResource, AppError> {
    notes::access(db, user, id, false).await?;
    let input = input?;
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
    validate_locale(&input.locale)?;
    for item in &input.texts {
        validate_text(item)?;
    }
    services.batch(&input.texts, &input.locale).await
}
pub async fn translate(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    services: &LearningServices,
    input: Result<Text, AppError>,
) -> Result<TranslationResource, AppError> {
    notes::access(db, user, id, false).await?;
    let input = input?;
    validate_text(&input.text)?;
    services.translate(input.text.trim()).await
}
