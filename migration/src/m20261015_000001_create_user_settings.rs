use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserSettings::Table)
                    .col(
                        ColumnDef::new(UserSettings::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSettings::Category)
                            .string_len(60)
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserSettings::Value).json_binary().not_null())
                    .col(
                        ColumnDef::new(UserSettings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserSettings::UserId)
                            .col(UserSettings::Category),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_settings_user_fk")
                            .from(UserSettings::Table, UserSettings::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserSettings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserSettings {
    Table,
    UserId,
    Category,
    Value,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
