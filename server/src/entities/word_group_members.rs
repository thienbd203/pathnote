use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "word_group_members")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub group_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub card_id: i64,
    pub position: i32,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
