pub(super) use super::answer::answer;
use super::{create as creation, dto::*, state};
use crate::AppError;
use sea_orm::DatabaseConnection;
pub async fn create(
    db: &DatabaseConnection,
    user: i64,
    input: Create,
) -> Result<ReviewResource, AppError> {
    Ok(creation::create(db, user, input).await?.state())
}
pub async fn show(db: &DatabaseConnection, user: i64, id: i64) -> Result<ReviewResource, AppError> {
    Ok(state::load(db, user, id).await?.state())
}
pub async fn pronunciation(
    db: &DatabaseConnection,
    user: i64,
    id: i64,
    input: Position,
) -> Result<Pronunciation, AppError> {
    let review = state::load(db, user, id).await?;
    if review.mode != Mode::Typing || input.position != review.answers.len() {
        return Err(AppError::Conflict("Câu hỏi đã thay đổi.".into()));
    }
    let q = review
        .questions
        .get(input.position)
        .ok_or_else(|| AppError::Conflict("Lượt ôn đã hoàn thành.".into()))?;
    Ok(Pronunciation {
        text: q.card.word.clone(),
        locale: q.card.locale.clone(),
    })
}
