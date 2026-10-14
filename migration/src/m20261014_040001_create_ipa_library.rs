use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Library::Table)
                .col(
                    ColumnDef::new(Library::Id)
                        .big_integer()
                        .auto_increment()
                        .primary_key()
                        .not_null(),
                )
                .col(ColumnDef::new(Library::UserId).big_integer().not_null())
                .col(ColumnDef::new(Library::Locale).string_len(5).not_null())
                .col(ColumnDef::new(Library::Kind).string_len(5).not_null())
                .col(ColumnDef::new(Library::Key).string_len(64).not_null())
                .col(ColumnDef::new(Library::Content).json_binary().not_null())
                .index(
                    Index::create()
                        .name("ipa_library_identity")
                        .unique()
                        .col(Library::UserId)
                        .col(Library::Locale)
                        .col(Library::Kind)
                        .col(Library::Key),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Library::Table, Library::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Library::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Library {
    #[sea_orm(iden = "ipa_library")]
    Table,
    Id,
    UserId,
    Locale,
    Kind,
    Key,
    Content,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
