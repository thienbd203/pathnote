use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Settings::Table)
                .col(ColumnDef::new(Settings::UserId).big_integer().not_null())
                .col(ColumnDef::new(Settings::Scope).string_len(64).not_null())
                .col(ColumnDef::new(Settings::Direction).string_len(5).not_null())
                .col(ColumnDef::new(Settings::Retention).float().not_null())
                .primary_key(Index::create().col(Settings::UserId).col(Settings::Scope))
                .foreign_key(
                    ForeignKey::create()
                        .from(Settings::Table, Settings::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(Memory::Table)
                .col(ColumnDef::new(Memory::UserId).big_integer().not_null())
                .col(ColumnDef::new(Memory::CardId).big_integer().not_null())
                .col(ColumnDef::new(Memory::Direction).string_len(5).not_null())
                .col(ColumnDef::new(Memory::Identity).json_binary().not_null())
                .col(ColumnDef::new(Memory::StudyKey).string_len(64).not_null())
                .col(ColumnDef::new(Memory::Stability).float().not_null())
                .col(ColumnDef::new(Memory::Difficulty).float().not_null())
                .col(ColumnDef::new(Memory::Step).integer().not_null())
                .col(ColumnDef::new(Memory::Reviews).integer().not_null())
                .col(ColumnDef::new(Memory::Lapses).integer().not_null())
                .col(
                    ColumnDef::new(Memory::DueAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Memory::ReviewedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(Memory::UserId)
                        .col(Memory::CardId)
                        .col(Memory::Direction),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Memory::Table, Memory::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Memory::Table, Memory::CardId)
                        .to(Cards::Table, Cards::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_due")
                .table(Memory::Table)
                .col(Memory::UserId)
                .col(Memory::Direction)
                .col(Memory::DueAt)
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(Attempts::Table)
                .col(ColumnDef::new(Attempts::Id).uuid().primary_key().not_null())
                .col(ColumnDef::new(Attempts::UserId).big_integer().not_null())
                .col(ColumnDef::new(Attempts::CardId).big_integer().not_null())
                .col(ColumnDef::new(Attempts::Direction).string_len(5).not_null())
                .col(ColumnDef::new(Attempts::StudyKey).string_len(64).not_null())
                .col(ColumnDef::new(Attempts::Version).integer().not_null())
                .col(ColumnDef::new(Attempts::Retention).float().not_null())
                .col(ColumnDef::new(Attempts::Revealed).boolean().not_null())
                .col(ColumnDef::new(Attempts::Rating).integer())
                .col(ColumnDef::new(Attempts::DueAt).timestamp_with_time_zone())
                .col(ColumnDef::new(Attempts::ReviewedAt).timestamp_with_time_zone())
                .col(
                    ColumnDef::new(Attempts::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Attempts::Table, Attempts::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Attempts::Table, Attempts::CardId)
                        .to(Cards::Table, Cards::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Attempts::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Memory::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Settings::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Settings {
    #[sea_orm(iden = "flashcard_settings")]
    Table,
    UserId,
    Scope,
    Direction,
    Retention,
}
#[derive(DeriveIden)]
enum Memory {
    #[sea_orm(iden = "flashcard_memory")]
    Table,
    UserId,
    CardId,
    Direction,
    Identity,
    StudyKey,
    Stability,
    Difficulty,
    Step,
    Reviews,
    Lapses,
    DueAt,
    ReviewedAt,
}
#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "flashcard_attempts")]
    Table,
    Id,
    UserId,
    CardId,
    Direction,
    StudyKey,
    Version,
    Retention,
    Revealed,
    Rating,
    DueAt,
    CreatedAt,
    ReviewedAt,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Cards {
    #[sea_orm(iden = "vocabulary_cards")]
    Table,
    Id,
}
