use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Models::Table)
                .col(
                    ColumnDef::new(Models::UserId)
                        .big_integer()
                        .primary_key()
                        .not_null(),
                )
                .col(ColumnDef::new(Models::Parameters).json_binary().not_null())
                .col(ColumnDef::new(Models::Version).integer().not_null())
                .col(ColumnDef::new(Models::TrainedReviews).integer().not_null())
                .col(ColumnDef::new(Models::TrainingSamples).integer().not_null())
                .col(ColumnDef::new(Models::BaselineLoss).float().not_null())
                .col(ColumnDef::new(Models::FittedLoss).float().not_null())
                .col(ColumnDef::new(Models::OptimizedAt).timestamp_with_time_zone())
                .col(
                    ColumnDef::new(Models::CheckedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Models::Table, Models::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        for table in [Memory::Table.into_iden(), Attempts::Table.into_iden()] {
            m.alter_table(
                Table::alter()
                    .table(table)
                    .add_column(
                        ColumnDef::new(Alias::new("parameter_version"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        }
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .add_column(ColumnDef::new(Attempts::DeltaDays).integer())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .add_column(ColumnDef::new(Attempts::Parameters).json_binary())
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_history")
                .table(Attempts::Table)
                .col(Attempts::UserId)
                .col(Attempts::CardId)
                .col(Attempts::Direction)
                .col(Attempts::ReviewedAt)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_index(
            Index::drop()
                .name("flashcard_history")
                .table(Attempts::Table)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .drop_column(Attempts::Parameters)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Attempts::Table)
                .drop_column(Attempts::DeltaDays)
                .to_owned(),
        )
        .await?;
        for table in [Memory::Table.into_iden(), Attempts::Table.into_iden()] {
            m.alter_table(
                Table::alter()
                    .table(table)
                    .drop_column(Alias::new("parameter_version"))
                    .to_owned(),
            )
            .await?;
        }
        m.drop_table(Table::drop().table(Models::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Models {
    #[sea_orm(iden = "flashcard_models")]
    Table,
    UserId,
    Parameters,
    Version,
    TrainedReviews,
    TrainingSamples,
    BaselineLoss,
    FittedLoss,
    OptimizedAt,
    CheckedAt,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Memory {
    #[sea_orm(iden = "flashcard_memory")]
    Table,
}
#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "flashcard_attempts")]
    Table,
    UserId,
    CardId,
    Direction,
    ReviewedAt,
    DeltaDays,
    Parameters,
}
