use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use crate::{
    entities::{prelude::Users, users},
    error::AppError,
};

pub async fn find_or_create_by_email(
    db: &DatabaseConnection,
    email: &str,
    name: Option<String>,
) -> Result<users::Model, AppError> {
    if let Some(user) = Users::find()
        .filter(users::Column::Email.eq(email))
        .one(db)
        .await?
    {
        return Ok(user);
    }

    let name = name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| email.split('@').next().unwrap_or(email).to_string());

    let user = users::ActiveModel {
        name: Set(name),
        email: Set(email.to_string()),
        ..Default::default()
    };

    Ok(user.insert(db).await?)
}

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<users::Model, AppError> {
    Users::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy người dùng".to_string()))
}
