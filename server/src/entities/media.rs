use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "media")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: i64,
    pub kind: String,
    pub name: String,
    pub storage_key: String,
    pub mime_type: String,
    pub size: i64,
    pub created_at: DateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sessions::Entity",
        from = "Column::SessionId",
        to = "super::sessions::Column::Id"
    )]
    Session,
}
impl ActiveModelBehavior for ActiveModel {}
