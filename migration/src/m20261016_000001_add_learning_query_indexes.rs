use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_index(
            Index::create()
                .name("ai_imports_session")
                .table(AiImports::Table)
                .col(AiImports::SessionId)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ai_imports_user")
                .table(AiImports::Table)
                .col(AiImports::UserId)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("exercises_user")
                .table(Exercises::Table)
                .col(Exercises::UserId)
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("flashcard_attempts_user_reviewed")
                .table(Attempts::Table)
                .col(Attempts::UserId)
                .col((Attempts::ReviewedAt, IndexOrder::Desc))
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for name in [
            "flashcard_attempts_user_reviewed",
            "exercises_user",
            "ai_imports_user",
            "ai_imports_session",
        ] {
            m.drop_index(Index::drop().name(name).to_owned()).await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum AiImports {
    Table,
    SessionId,
    UserId,
}

#[derive(DeriveIden)]
enum Exercises {
    Table,
    UserId,
}

#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "flashcard_attempts")]
    Table,
    UserId,
    ReviewedAt,
}
