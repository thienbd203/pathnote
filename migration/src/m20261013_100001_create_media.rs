use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Media::Table)
                .col(
                    ColumnDef::new(Media::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(Media::SessionId).big_integer().not_null())
                .col(ColumnDef::new(Media::Kind).string_len(10).not_null())
                .col(ColumnDef::new(Media::Name).string_len(200).not_null())
                .col(
                    ColumnDef::new(Media::StorageKey)
                        .string_len(36)
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(Media::MimeType).string_len(100).not_null())
                .col(ColumnDef::new(Media::Size).big_integer().not_null())
                .col(
                    ColumnDef::new(Media::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Media::Table, Media::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::col(Media::Kind).is_in(["image", "file"]))
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("media_session_kind")
                .table(Media::Table)
                .col(Media::SessionId)
                .col(Media::Kind)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Media::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Media {
    Table,
    Id,
    SessionId,
    Kind,
    Name,
    StorageKey,
    MimeType,
    Size,
    CreatedAt,
}
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
