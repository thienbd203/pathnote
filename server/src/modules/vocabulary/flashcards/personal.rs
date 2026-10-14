use super::history;
use crate::{
    entities::{flashcard_memory as memory, flashcard_models as models},
    AppError,
};
use fsrs::FSRS;
use sea_orm::{ConnectionTrait, EntityTrait};
pub struct Profile {
    pub parameter_version: i32,
    pub model: Option<models::Model>,
    pub parameters: Vec<f32>,
}
impl Profile {
    pub fn version(&self) -> i32 {
        self.parameter_version
    }
    pub fn scheduler(&self) -> Result<FSRS, AppError> {
        FSRS::new(&self.parameters)
            .map_err(|_| AppError::Internal("Tham số FSRS không hợp lệ.".into()))
    }
    pub async fn memory<C: ConnectionTrait>(
        &self,
        db: &C,
        user: i64,
        m: Option<memory::Model>,
    ) -> Result<Option<memory::Model>, AppError> {
        let Some(mut m) = m else { return Ok(None) };
        if m.parameter_version != self.version() {
            let item = history::current(db, user, m.card_id, &m.direction, &m.study_key, m.reviews)
                .await?;
            let state = self
                .scheduler()?
                .memory_state(item, None)
                .map_err(|_| AppError::Internal("Không khôi phục được trí nhớ FSRS.".into()))?;
            m.stability = state.stability;
            m.difficulty = state.difficulty;
            m.parameter_version = self.version();
        }
        Ok(Some(m))
    }
}
pub async fn load<C: ConnectionTrait>(db: &C, user: i64) -> Result<Profile, AppError> {
    let model = models::Entity::find_by_id(user).one(db).await?;
    let parameters = match &model {
        Some(m) => serde_json::from_value(m.parameters.clone())
            .map_err(|_| AppError::Internal("Không đọc được tham số FSRS.".into()))?,
        None => fsrs::DEFAULT_PARAMETERS.to_vec(),
    };
    let profile = Profile {
        parameter_version: model.as_ref().map_or(0, |m| m.version),
        model,
        parameters,
    };
    profile.scheduler()?;
    Ok(profile)
}

pub fn for_ticket(
    ticket: &crate::entities::flashcard_attempts::Model,
) -> Result<Profile, AppError> {
    let parameters = match &ticket.parameters {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|_| AppError::Internal("Không đọc được tham số thẻ.".into()))?,
        None if ticket.parameter_version == 0 => fsrs::DEFAULT_PARAMETERS.to_vec(),
        None => return Err(AppError::Internal("Thẻ thiếu tham số FSRS.".into())),
    };
    Ok(Profile {
        parameter_version: ticket.parameter_version,
        model: None,
        parameters,
    })
}
