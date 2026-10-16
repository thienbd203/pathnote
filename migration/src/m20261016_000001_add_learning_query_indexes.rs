use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            CREATE INDEX ai_imports_session ON ai_imports(session_id);
            CREATE INDEX ai_imports_user ON ai_imports(user_id);
            CREATE INDEX exercises_user ON exercises(user_id);
            CREATE INDEX flashcard_attempts_user_reviewed ON flashcard_attempts(user_id, reviewed_at DESC);
        "#).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
            DROP INDEX flashcard_attempts_user_reviewed;
            DROP INDEX exercises_user;
            DROP INDEX ai_imports_user;
            DROP INDEX ai_imports_session;
        "#,
            )
            .await?;
        Ok(())
    }
}
