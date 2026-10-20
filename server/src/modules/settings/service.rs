use super::dto::*;
use crate::{entities::user_settings, AppError};
use sea_orm::{sea_query::OnConflict, EntityTrait, Set};
use std::collections::HashSet;
pub async fn read(
    db: &sea_orm::DatabaseConnection,
    user: i64,
) -> Result<PersonalSettings, AppError> {
    let saved = user_settings::Entity::find_by_id((user, "learning".to_owned()))
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
    // 1. Kiểm tra cài đặt trước khi ghi database.
    learning.validate()?;

    // 2. Chuyển cài đặt sang JSON; trả lỗi nếu không chuyển được.
    let learning_json = match serde_json::to_value(&learning) {
        Ok(value) => value,
        Err(_) => {
            return Err(AppError::Internal(
                "Không lưu được cài đặt học tập.".to_owned(),
            ));
        }
    };

    // 3. Chuẩn bị dữ liệu cho bản ghi mới.
    let now = chrono::Utc::now().fixed_offset();
    let settings = user_settings::ActiveModel {
        user_id: Set(user_id),
        category: Set("learning".to_owned()),
        value: Set(learning_json),
        updated_at: Set(now),
    };

    // 4. Nếu trùng (user_id, category), chỉ cập nhật giá trị và thời gian.
    let conflict_policy = OnConflict::columns([
        user_settings::Column::UserId,
        user_settings::Column::Category,
    ])
    .update_columns([
        user_settings::Column::Value,
        user_settings::Column::UpdatedAt,
    ])
    .to_owned();

    // 5. Thực thi một câu lệnh thêm hoặc cập nhật rồi trả cài đặt đã nhận.
    let query = user_settings::Entity::insert(settings).on_conflict(conflict_policy);
    query.exec(db).await?;

    Ok(PersonalSettings { learning })
}
pub async fn read_navigation(
    db: &sea_orm::DatabaseConnection,
    user: i64,
) -> Result<NavigationSettings, AppError> {
    let saved = user_settings::Entity::find_by_id((user, "navigation".to_owned()))
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
    user_settings::Entity::insert(user_settings::ActiveModel {
        user_id: Set(user),
        category: Set("navigation".into()),
        value: Set(serde_json::to_value(&value)
            .map_err(|_| AppError::Internal("Không lưu được bộ lọc.".into()))?),
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
    Ok(value)
}
