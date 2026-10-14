use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "exercises")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: i64,
    pub user_id: i64,
    pub title: String,
    pub question: Json,
    pub revision: i32,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
