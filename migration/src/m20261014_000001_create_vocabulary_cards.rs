use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Cards::Table)
                .col(
                    ColumnDef::new(Cards::Id)
                        .big_integer()
                        .auto_increment()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Cards::SessionId).big_integer().not_null())
                .col(
                    ColumnDef::new(Cards::SourceEntryId)
                        .string_len(36)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cards::SourceSenseId)
                        .string_len(36)
                        .not_null(),
                )
                .col(ColumnDef::new(Cards::Word).text().not_null())
                .col(ColumnDef::new(Cards::WordKey).string_len(64).not_null())
                .col(ColumnDef::new(Cards::Locale).string_len(10).not_null())
                .col(ColumnDef::new(Cards::Ipa).text().not_null())
                .col(
                    ColumnDef::new(Cards::PartOfSpeech)
                        .string_len(20)
                        .not_null(),
                )
                .col(ColumnDef::new(Cards::Meaning).text().not_null())
                .col(ColumnDef::new(Cards::Topics).json_binary().not_null())
                .col(ColumnDef::new(Cards::Examples).json_binary().not_null())
                .col(
                    ColumnDef::new(Cards::Active)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(Cards::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Cards::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Cards::Table, Cards::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .index(
                    Index::create()
                        .name("vocabulary_source_sense")
                        .unique()
                        .col(Cards::SessionId)
                        .col(Cards::SourceSenseId),
                )
                .check(Expr::col(Cards::Locale).is_in(["en-US", "en-GB"]))
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("vocabulary_session_active")
                .table(Cards::Table)
                .col(Cards::SessionId)
                .col(Cards::Active)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Cards::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Cards {
    #[sea_orm(iden = "vocabulary_cards")]
    Table,
    Id,
    SessionId,
    SourceEntryId,
    SourceSenseId,
    Word,
    WordKey,
    Locale,
    Ipa,
    PartOfSpeech,
    Meaning,
    Topics,
    Examples,
    Active,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
