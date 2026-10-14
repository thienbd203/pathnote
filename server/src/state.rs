use crate::{config::Config, modules::auth::google::GoogleAuth};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub learning_services: Arc<crate::LearningServices>,
    pub db: Arc<DatabaseConnection>,
    pub access_tokens: Arc<crate::access_token::AccessTokens>,
    pub google: Arc<GoogleAuth>,
    pub config: Arc<Config>,
}
