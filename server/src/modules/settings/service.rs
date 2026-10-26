use super::dto::*;
use crate::{entities::user_settings, AppError};
use sea_orm::{sea_query::OnConflict, EntityTrait, Set};
use std::collections::HashSet;
#[derive(Clone, Copy)]
enum Category {
    Learning,
    Navigation,
}
impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Self::Learning => "learning",
            Self::Navigation => "navigation",
        }
    }
}
pub async fn read(
    db: &sea_orm::DatabaseConnection,
    user: i64,
) -> Result<PersonalSettings, AppError> {
    let saved = user_settings::Entity::find_by_id((user, Category::Learning.as_str().to_owned()))
        .one(db)
        .await?;
    let learning = match saved {
        Some(saved) => serde_json::from_value(saved.value)
            .map_err(|_| AppError::Internal("Không đọc được cài đặt học tập.".into()))?,
        None => LearningSettings::default(),
    };
    Ok(PersonalSettings { learning })
}

pub async fn save_learning(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    learning: LearningSettings,
) -> Result<PersonalSettings, AppError> {
    learning.validate()?;
    persist(
        db,
        user_id,
        Category::Learning,
        &learning,
        "Không lưu được cài đặt học tập.",
    )
    .await?;
    Ok(PersonalSettings { learning })
}
pub async fn read_navigation(
    db: &sea_orm::DatabaseConnection,
    user: i64,
) -> Result<NavigationSettings, AppError> {
    let saved = user_settings::Entity::find_by_id((user, Category::Navigation.as_str().to_owned()))
        .one(db)
        .await?;
    let value = match saved {
        Some(saved) => serde_json::from_value(saved.value)
            .map_err(|_| AppError::Internal("Không đọc được bộ lọc cá nhân.".into()))?,
        None => NavigationSettings::default(),
    };
    Ok(value)
}
pub async fn save_navigation(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    value: NavigationSettings,
) -> Result<NavigationSettings, AppError> {
    if value.hidden_chips.len() > 100
        || value.pinned_places.len() > 100
        || value.hidden_chips.iter().any(|chip| chip.id <= 0)
        || value.pinned_places.iter().any(|id| *id <= 0)
        || value.hidden_chips.iter().collect::<HashSet<_>>().len() != value.hidden_chips.len()
        || value.pinned_places.iter().collect::<HashSet<_>>().len() != value.pinned_places.len()
    {
        return Err(AppError::validation(
            "navigation",
            "Mỗi danh sách tối đa 100 mục, ID dương và không trùng nhau.",
        ));
    }
    persist(
        db,
        user,
        Category::Navigation,
        &value,
        "Không lưu được bộ lọc.",
    )
    .await?;
    Ok(value)
}

async fn persist<T: serde::Serialize>(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    category: Category,
    value: &T,
    message: &str,
) -> Result<(), AppError> {
    let value = serde_json::to_value(value).map_err(|_| AppError::Internal(message.into()))?;
    user_settings::Entity::insert(user_settings::ActiveModel {
        user_id: Set(user),
        category: Set(category.as_str().into()),
        value: Set(value),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    })
    .on_conflict(
        OnConflict::columns([
            user_settings::Column::UserId,
            user_settings::Column::Category,
        ])
        .update_columns([
            user_settings::Column::Value,
            user_settings::Column::UpdatedAt,
        ])
        .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}
