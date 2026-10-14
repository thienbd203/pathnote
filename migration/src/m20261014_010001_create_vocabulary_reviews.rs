use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Reviews::Table)
                .col(
                    ColumnDef::new(Reviews::Id)
                        .big_integer()
                        .auto_increment()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Reviews::UserId).big_integer().not_null())
                .col(ColumnDef::new(Reviews::Mode).string_len(10).not_null())
                .col(ColumnDef::new(Reviews::Filters).json_binary().not_null())
                .col(ColumnDef::new(Reviews::Questions).json_binary().not_null())
                .col(ColumnDef::new(Reviews::Answers).json_binary().not_null())
                .col(
                    ColumnDef::new(Reviews::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Reviews::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Reviews::Table, Reviews::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::col(Reviews::Mode).is_in(["typing", "choice"]))
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("vocabulary_reviews_user")
                .table(Reviews::Table)
                .col(Reviews::UserId)
                .col(Reviews::Id)
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(Progress::Table)
                .col(ColumnDef::new(Progress::UserId).big_integer().not_null())
                .col(ColumnDef::new(Progress::CardId).big_integer().not_null())
                .col(ColumnDef::new(Progress::StudyKey).string_len(64).not_null())
                .col(
                    ColumnDef::new(Progress::Correct)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Progress::Incorrect)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Progress::Streak)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Progress::DueAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Progress::LastReviewedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .primary_key(Index::create().col(Progress::UserId).col(Progress::CardId))
                .foreign_key(
                    ForeignKey::create()
                        .from(Progress::Table, Progress::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Progress::Table, Progress::CardId)
                        .to(Cards::Table, Cards::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Progress::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Reviews::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Reviews {
    #[sea_orm(iden = "vocabulary_reviews")]
    Table,
    Id,
    UserId,
    Mode,
    Filters,
    Questions,
    Answers,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Progress {
    #[sea_orm(iden = "vocabulary_progress")]
    Table,
    UserId,
    CardId,
    StudyKey,
    Correct,
    Incorrect,
    Streak,
    DueAt,
    LastReviewedAt,
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
