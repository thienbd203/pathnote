use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Models::Table)
                .add_column(ColumnDef::new(Models::EvaluatedAt).timestamp_with_time_zone())
                .to_owned(),
        )
        .await?;
        let query = Query::update()
            .table(Models::Table)
            .value(Models::EvaluatedAt, Expr::col(Models::CheckedAt))
            .and_where(Expr::col(Models::TrainingSamples).gt(0))
            .to_owned();
        m.get_connection()
            .execute(m.get_database_backend().build(&query))
            .await?;
        Ok(())
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Models::Table)
                .drop_column(Models::EvaluatedAt)
                .to_owned(),
        )
        .await
    }
}
#[derive(DeriveIden)]
enum Models {
    #[sea_orm(iden = "flashcard_models")]
    Table,
    EvaluatedAt,
    CheckedAt,
    TrainingSamples,
}
