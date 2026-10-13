use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "courses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub place_id: i64,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub starts_on: Date,
    pub ends_on: Option<Date>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::places::Entity",
        from = "Column::PlaceId",
        to = "super::places::Column::Id"
    )]
    Place,
}
impl Related<super::places::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Place.def()
    }
}
impl ActiveModelBehavior for ActiveModel {}
