use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Alias::new("flashcard_attempts"))
                .add_column(ColumnDef::new(Alias::new("snapshot")).json_binary())
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Alias::new("flashcard_attempts"))
                .drop_column(Alias::new("snapshot"))
                .to_owned(),
        )
        .await
    }
}
