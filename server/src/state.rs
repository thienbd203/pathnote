use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::auth::JwtKeys;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub jwt: Arc<JwtKeys>,
}
