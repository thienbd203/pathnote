use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "exercise_attempts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub exercise_id: i64,
    pub user_id: i64,
    pub revision: i32,
    pub result: Option<Json>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
