use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flashcard_memory")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    pub card_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub study_key: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub direction: String,
    pub identity: Json,
    pub parameter_version: i32,
    pub stability: f32,
    pub difficulty: f32,
    pub step: i32,
    pub reviews: i32,
    pub lapses: i32,
    pub due_at: DateTimeWithTimeZone,
    pub reviewed_at: DateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
