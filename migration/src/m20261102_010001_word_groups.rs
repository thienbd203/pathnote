use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Groups::Table)
                .col(
                    ColumnDef::new(Groups::Id)
                        .big_integer()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(Groups::UserId).big_integer().not_null())
                .col(ColumnDef::new(Groups::Name).string_len(100).not_null())
                .col(ColumnDef::new(Groups::Note).text().not_null())
                .col(
                    ColumnDef::new(Groups::Revision)
                        .integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Groups::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Groups::Table, Groups::UserId)
                        .to(Alias::new("users"), Alias::new("id"))
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(Members::Table)
                .col(ColumnDef::new(Members::GroupId).big_integer().not_null())
                .col(ColumnDef::new(Members::CardId).big_integer().not_null())
                .col(ColumnDef::new(Members::Position).integer().not_null())
                .primary_key(Index::create().col(Members::GroupId).col(Members::CardId))
                .foreign_key(
                    ForeignKey::create()
                        .from(Members::Table, Members::GroupId)
                        .to(Groups::Table, Groups::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Members::Table, Members::CardId)
                        .to(Alias::new("vocabulary_cards"), Alias::new("id"))
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("word_groups_user")
                .table(Groups::Table)
                .col(Groups::UserId)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("word_group_members_card")
                .table(Members::Table)
                .col(Members::CardId)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Members::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Groups::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Groups {
    #[sea_orm(iden = "word_groups")]
    Table,
    Id,
    UserId,
    Name,
    Note,
    Revision,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Members {
    #[sea_orm(iden = "word_group_members")]
    Table,
    GroupId,
    CardId,
    Position,
}
