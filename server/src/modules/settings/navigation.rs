use crate::{
    access_token::CurrentUser, entities::user_settings, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use sea_orm::{sea_query::OnConflict, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipKind {
    Course,
    Place,
}
#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HiddenChip {
    kind: ChipKind,
    id: i64,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSettings {
    hidden_chips: Vec<HiddenChip>,
    pinned_places: Vec<i64>,
}
pub async fn read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<NavigationSettings>, AppError> {
    let saved = user_settings::Entity::find_by_id((user.id, "navigation".to_owned()))
        .one(state.db.as_ref())
        .await?;
    let value = match saved {
        Some(saved) => serde_json::from_value(saved.value)
            .map_err(|_| AppError::Internal("Không đọc được bộ lọc cá nhân.".into()))?,
        None => NavigationSettings::default(),
    };
    Ok(ApiResponse::ok(value))
}
pub async fn save(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<Json<NavigationSettings>, JsonRejection>,
) -> Result<ApiResponse<NavigationSettings>, AppError> {
    let Json(value) =
        input.map_err(|_| AppError::validation("navigation", "Bộ lọc cá nhân không hợp lệ."))?;
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
        user_id: Set(user.id),
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
    .exec(state.db.as_ref())
    .await?;
    Ok(ApiResponse::ok(value))
}
