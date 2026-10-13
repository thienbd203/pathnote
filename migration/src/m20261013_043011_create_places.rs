use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Places::Table)
                    .col(
                        ColumnDef::new(Places::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Places::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Places::Name).string_len(200).not_null())
                    .col(ColumnDef::new(Places::Description).text())
                    .col(
                        ColumnDef::new(Places::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Places::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Places::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("places_user_fk")
                            .from(Places::Table, Places::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("places_owner_index")
                    .table(Places::Table)
                    .col(Places::UserId)
                    .col(Places::DeletedAt)
                    .col(Places::Id)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Places::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Places {
    Table,
    Id,
    UserId,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
