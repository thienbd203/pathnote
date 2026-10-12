use super::google::GoogleProfile;
use crate::{entities::users, error::AppError};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    EntityTrait, QueryFilter,
};

pub async fn find_or_create_google_user(
    db: &DatabaseConnection,
    profile: GoogleProfile,
) -> Result<users::Model, AppError> {
    // Identity is Google's subject, never an email supplied by the browser.
    if let Some(owner) = users::Entity::find()
        .filter(users::Column::Email.eq(&profile.email))
        .one(db)
        .await?
    {
        if owner.google_id.as_deref() != Some(&profile.subject) {
            return Err(AppError::BadRequest(
                "Email đã thuộc một tài khoản khác".into(),
            ));
        }
    }
    let model = users::ActiveModel {
        name: Set(profile.name.clone()),
        email: Set(profile.email.clone()),
        google_id: Set(Some(profile.subject.clone())),
        avatar_url: Set(profile.avatar_url.clone()),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    };
    let result = users::Entity::insert(model)
        .on_conflict(
            OnConflict::column(users::Column::GoogleId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await;
    if let Err(error) = result {
        // A concurrent first login may have inserted this same Google identity.
        if users::Entity::find()
            .filter(users::Column::GoogleId.eq(&profile.subject))
            .one(db)
            .await?
            .is_none()
        {
            return Err(error.into());
        }
    }
    let user = users::Entity::find()
        .filter(users::Column::GoogleId.eq(&profile.subject))
        .one(db)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    let mut model: users::ActiveModel = user.into();
    model.name = Set(profile.name);
    model.email = Set(profile.email);
    model.avatar_url = Set(profile.avatar_url);
    model.updated_at = Set(chrono::Utc::now().fixed_offset());
    Ok(model.update(db).await?)
}
