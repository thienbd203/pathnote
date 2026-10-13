use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[sea_orm(string_value = "scheduled")]
    Scheduled,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub place_id: i64,
    pub course_id: Option<i64>,
    pub title: String,
    pub starts_at: DateTimeWithTimeZone,
    pub duration_minutes: i16,
    pub status: SessionStatus,
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
    #[sea_orm(
        belongs_to = "super::courses::Entity",
        from = "(Column::CourseId,Column::PlaceId)",
        to = "(super::courses::Column::Id,super::courses::Column::PlaceId)"
    )]
    Course,
}
impl Related<super::places::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Place.def()
    }
}
impl Related<super::courses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Course.def()
    }
}
impl ActiveModelBehavior for ActiveModel {}

// A session has many collaborators through the junction table.
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        super::note_collaborators::Relation::User.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::note_collaborators::Relation::Session.def().rev())
    }
}
