use serde::Serialize;

use crate::entities::users;

#[derive(Serialize)]
pub struct UserResource {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
}

impl From<users::Model> for UserResource {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
        }
    }
}

#[derive(Serialize)]
pub struct TokenResource {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub user: UserResource,
}
