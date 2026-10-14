use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flashcard_models")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    pub parameters: Json,
    pub version: i32,
    pub trained_reviews: i32,
    pub training_samples: i32,
    pub baseline_loss: f32,
    pub fitted_loss: f32,
    pub optimized_at: Option<DateTimeWithTimeZone>,
    pub checked_at: DateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
