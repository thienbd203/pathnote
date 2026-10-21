use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(MediaProgress::Table)
                .col(
                    ColumnDef::new(MediaProgress::UserId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MediaProgress::MediaId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MediaProgress::PositionSeconds)
                        .double()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(MediaProgress::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(MediaProgress::UserId)
                        .col(MediaProgress::MediaId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("media_progress_user_fk")
                        .from(MediaProgress::Table, MediaProgress::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("media_progress_media_fk")
                        .from(MediaProgress::Table, MediaProgress::MediaId)
                        .to(Media::Table, Media::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::col(MediaProgress::PositionSeconds).gte(0))
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(MediaProgress::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum MediaProgress {
    Table,
    UserId,
    MediaId,
    PositionSeconds,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Media {
    Table,
    Id,
}
