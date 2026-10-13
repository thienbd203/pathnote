use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Courses::Table)
                    .col(
                        ColumnDef::new(Courses::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Courses::PlaceId).big_integer().not_null())
                    .col(ColumnDef::new(Courses::Name).string_len(200).not_null())
                    .col(ColumnDef::new(Courses::Description).text())
                    .col(ColumnDef::new(Courses::StartsOn).date().not_null())
                    .col(ColumnDef::new(Courses::EndsOn).date())
                    .col(
                        ColumnDef::new(Courses::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Courses::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Courses::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("courses_place_fk")
                            .from(Courses::Table, Courses::PlaceId)
                            .to(Places::Table, Places::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(
                        Expr::col(Courses::EndsOn)
                            .is_null()
                            .or(Expr::col(Courses::EndsOn).gte(Expr::col(Courses::StartsOn))),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("courses_place_index")
                    .table(Courses::Table)
                    .col(Courses::PlaceId)
                    .col(Courses::DeletedAt)
                    .col(Courses::Id)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Courses::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Courses {
    Table,
    Id,
    PlaceId,
    Name,
    Description,
    StartsOn,
    EndsOn,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}
#[derive(DeriveIden)]
enum Places {
    Table,
    Id,
}
