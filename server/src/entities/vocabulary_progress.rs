use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "vocabulary_progress")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub card_id: i64,
    pub study_key: String,
    pub correct: i32,
    pub incorrect: i32,
    pub streak: i32,
    pub due_at: DateTimeWithTimeZone,
    pub last_reviewed_at: DateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::vocabulary_cards::Entity",
        from = "Column::CardId",
        to = "super::vocabulary_cards::Column::Id"
    )]
    Card,
}
impl ActiveModelBehavior for ActiveModel {}
