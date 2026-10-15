use crate::{
    access_token::CurrentUser, entities::user_settings, response::ApiResponse, state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    routing::{get, put},
    Json, Router,
};
use chrono::{NaiveTime, Timelike};
use sea_orm::{sea_query::OnConflict, EntityTrait, Set};
use serde::{Deserialize, Serialize};

// Store categories independently so saving learning defaults cannot overwrite
// future appearance/notification preferences. Each category has a typed API.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningSettings {
    pub session_start_time: String,
    pub session_duration_minutes: u16,
}

impl Default for LearningSettings {
    fn default() -> Self {
        Self {
            session_start_time: "19:30".into(),
            session_duration_minutes: 60,
        }
    }
}

impl LearningSettings {
    fn validate(&self) -> Result<(), AppError> {
        let time = NaiveTime::parse_from_str(&self.session_start_time, "%H:%M")
            .ok()
            .filter(|_| self.session_start_time.len() == 5)
            .ok_or_else(|| {
                AppError::validation(
                    "session_start_time",
                    "Giờ bắt đầu phải có dạng HH:MM từ 00:00 đến 23:59.",
                )
            })?;
        if !(1..=1440).contains(&self.session_duration_minutes) {
            return Err(AppError::validation(
                "session_duration_minutes",
                "Thời lượng phải từ 1 đến 1440 phút.",
            ));
        }
        if time.hour() * 60 + time.minute() + u32::from(self.session_duration_minutes) >= 1440 {
            return Err(AppError::validation(
                "session_duration_minutes",
                "Buổi học phải kết thúc trong cùng ngày học.",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct PersonalSettings {
    pub learning: LearningSettings,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(read))
        .route("/settings/learning", put(save_learning))
        .layer(DefaultBodyLimit::max(4096))
}

async fn read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<PersonalSettings>, AppError> {
    let saved = user_settings::Entity::find_by_id((user.id, "learning".to_owned()))
        .one(state.db.as_ref())
        .await?;
    let learning = match saved {
        Some(saved) => serde_json::from_value(saved.value)
            .map_err(|_| AppError::Internal("Không đọc được cài đặt học tập.".into()))?,
        None => LearningSettings::default(),
    };
    Ok(ApiResponse::ok(PersonalSettings { learning }))
}

async fn save_learning(
    State(state): State<AppState>,
    user: CurrentUser,
    input: Result<Json<LearningSettings>, JsonRejection>,
) -> Result<ApiResponse<PersonalSettings>, AppError> {
    let Json(learning) =
        input.map_err(|_| AppError::validation("settings", "Cài đặt học tập không hợp lệ."))?;
    learning.validate()?;
    user_settings::Entity::insert(user_settings::ActiveModel {
        user_id: Set(user.id),
        category: Set("learning".into()),
        value: Set(serde_json::to_value(&learning)
            .map_err(|_| AppError::Internal("Không lưu được cài đặt học tập.".into()))?),
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
    Ok(ApiResponse::ok(PersonalSettings { learning }))
}
