use crate::{
    entities::{prelude::Users, users},
    error::AppError,
    modules::auth::{
        google::{GoogleAuth, GoogleCredentials},
        service,
    },
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;

pub type AuthSession = axum_login::AuthSession<Backend>;

impl AuthUser for users::Model {
    type Id = i64;
    fn id(&self) -> Self::Id {
        self.id
    }
    fn session_auth_hash(&self) -> &[u8] {
        self.google_id.as_deref().unwrap_or_default().as_bytes()
    }
}

#[derive(Clone)]
pub struct Backend {
    pub db: Arc<DatabaseConnection>,
    pub google: Arc<GoogleAuth>,
}
impl AuthnBackend for Backend {
    type User = users::Model;
    type Credentials = GoogleCredentials;
    type Error = AppError;
    async fn authenticate(
        &self,
        credentials: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let profile = self.google.verify(credentials).await?;
        service::find_or_create_google_user(self.db.as_ref(), profile)
            .await
            .map(Some)
    }
    async fn get_user(&self, id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(Users::find_by_id(*id)
            .one(self.db.as_ref())
            .await?
            .filter(|user| user.google_id.is_some()))
    }
}
