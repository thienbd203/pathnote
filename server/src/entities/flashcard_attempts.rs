use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flashcard_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: i64,
    pub card_id: i64,
    pub direction: String,
    pub study_key: String,
    pub version: i32,
    pub parameter_version: i32,
    pub parameters: Option<Json>,
    pub snapshot: Option<Json>,
    pub delta_days: Option<i32>,
    pub retention: f32,
    pub revealed: bool,
    pub rating: Option<i32>,
    pub due_at: Option<DateTimeWithTimeZone>,
    pub reviewed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::vocabulary_cards::Entity",
        from = "Column::CardId",
        to = "super::vocabulary_cards::Column::Id"
    )]
    Card,
}
impl ActiveModelBehavior for ActiveModel {}
