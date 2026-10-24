use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Runs::Table)
                .col(ColumnDef::new(Runs::Id).uuid().primary_key().not_null())
                .col(ColumnDef::new(Runs::UserId).big_integer().not_null())
                .col(ColumnDef::new(Runs::Filters).json_binary().not_null())
                .col(ColumnDef::new(Runs::Settings).json_binary().not_null())
                .col(
                    ColumnDef::new(Runs::InitialTotal)
                        .big_integer()
                        .not_null()
                        .check(Expr::col(Runs::InitialTotal).gte(0)),
                )
                .col(
                    ColumnDef::new(Runs::StartedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Runs::Completed)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Runs::FinishedAt).timestamp_with_time_zone())
                .foreign_key(
                    ForeignKey::create()
                        .from(Runs::Table, Runs::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_runs_user_started")
                .table(Runs::Table)
                .col(Runs::UserId)
                .col((Runs::StartedAt, IndexOrder::Desc))
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .add_column(ColumnDef::new(Attempts::RunId).uuid())
                .add_foreign_key(
                    TableForeignKey::new()
                        .name("flashcard_attempts_run_id_fkey")
                        .from_tbl(Attempts::Table)
                        .from_col(Attempts::RunId)
                        .to_tbl(Runs::Table)
                        .to_col(Runs::Id)
                        .on_delete(ForeignKeyAction::SetNull),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_attempts_run")
                .table(Attempts::Table)
                .col(Attempts::RunId)
                .col((Attempts::ReviewedAt, IndexOrder::Desc))
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .drop_column(Attempts::RunId)
                .to_owned(),
        )
        .await?;
        m.drop_table(Table::drop().table(Runs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Runs {
    #[sea_orm(iden = "flashcard_runs")]
    Table,
    Id,
    UserId,
    Filters,
    Settings,
    InitialTotal,
    StartedAt,
    Completed,
    FinishedAt,
}

#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "flashcard_attempts")]
    Table,
    RunId,
    ReviewedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
