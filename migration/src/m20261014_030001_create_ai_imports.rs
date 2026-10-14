use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Imports::Table)
                .col(ColumnDef::new(Imports::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Imports::SessionId).big_integer().not_null())
                .col(ColumnDef::new(Imports::UserId).big_integer().not_null())
                .col(
                    ColumnDef::new(Imports::Fingerprint)
                        .string_len(64)
                        .not_null(),
                )
                .col(ColumnDef::new(Imports::Content).json_binary().not_null())
                .col(
                    ColumnDef::new(Imports::ExerciseIds)
                        .json_binary()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Imports::Attached)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(Imports::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Imports::Table, Imports::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Imports::Table, Imports::UserId)
                        .to(Users::Table, Users::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ai_imports_pending")
                .table(Imports::Table)
                .col(Imports::Attached)
                .col(Imports::CreatedAt)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Imports::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Imports {
    #[sea_orm(iden = "ai_imports")]
    Table,
    Id,
    SessionId,
    UserId,
    Fingerprint,
    Content,
    ExerciseIds,
    Attached,
    CreatedAt,
}
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
