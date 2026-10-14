use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Exercises::Table)
                .col(
                    ColumnDef::new(Exercises::Id)
                        .big_integer()
                        .auto_increment()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Exercises::SessionId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Exercises::UserId).big_integer().not_null())
                .col(ColumnDef::new(Exercises::Title).string_len(200).not_null())
                .col(ColumnDef::new(Exercises::Question).json_binary().not_null())
                .col(ColumnDef::new(Exercises::Revision).integer().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(Exercises::Table, Exercises::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Exercises::Table, Exercises::UserId)
                        .to(Users::Table, Users::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(Attempts::Table)
                .col(
                    ColumnDef::new(Attempts::Id)
                        .big_integer()
                        .auto_increment()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Attempts::ExerciseId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Attempts::UserId).big_integer().not_null())
                .col(ColumnDef::new(Attempts::Revision).integer().not_null())
                .col(ColumnDef::new(Attempts::Result).json_binary())
                .foreign_key(
                    ForeignKey::create()
                        .from(Attempts::Table, Attempts::ExerciseId)
                        .to(Exercises::Table, Exercises::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Attempts::Table, Attempts::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("exercise_attempts_latest")
                .table(Attempts::Table)
                .col(Attempts::ExerciseId)
                .col(Attempts::UserId)
                .col(Attempts::Revision)
                .col(Attempts::Id)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("exercises_session")
                .table(Exercises::Table)
                .col(Exercises::SessionId)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Attempts::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Exercises::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Exercises {
    Table,
    Id,
    SessionId,
    UserId,
    Title,
    Question,
    Revision,
}
#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "exercise_attempts")]
    Table,
    Id,
    ExerciseId,
    UserId,
    Revision,
    Result,
}
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
