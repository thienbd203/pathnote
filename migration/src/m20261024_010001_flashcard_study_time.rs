use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(StudyTime::Table)
                .col(
                    ColumnDef::new(StudyTime::Id)
                        .uuid()
                        .primary_key()
                        .not_null(),
                )
                .col(ColumnDef::new(StudyTime::UserId).big_integer().not_null())
                .col(ColumnDef::new(StudyTime::RunId).uuid().not_null())
                .col(
                    ColumnDef::new(StudyTime::StartedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(StudyTime::EndedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                // Mirrors the API rule: one study segment lasts at most 15 seconds.
                .check(
                    Expr::col(StudyTime::EndedAt)
                        .gt(Expr::col(StudyTime::StartedAt))
                        .and(
                            Expr::col(StudyTime::EndedAt).lte(
                                Expr::col(StudyTime::StartedAt)
                                    .add(Expr::cust("INTERVAL '15 seconds'")),
                            ),
                        ),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(StudyTime::Table, StudyTime::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(StudyTime::Table, StudyTime::RunId)
                        .to(Runs::Table, Runs::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_study_time_user_start")
                .table(StudyTime::Table)
                .col(StudyTime::UserId)
                .col(StudyTime::StartedAt)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_study_time_run")
                .table(StudyTime::Table)
                .col(StudyTime::RunId)
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(StudyTime::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum StudyTime {
    #[sea_orm(iden = "flashcard_study_time")]
    Table,
    Id,
    UserId,
    RunId,
    StartedAt,
    EndedAt,
}

#[derive(DeriveIden)]
enum Runs {
    #[sea_orm(iden = "flashcard_runs")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
