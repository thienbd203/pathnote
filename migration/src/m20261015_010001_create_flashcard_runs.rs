use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            CREATE TABLE flashcard_runs (
                id UUID PRIMARY KEY,
                user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                filters JSONB NOT NULL,
                settings JSONB NOT NULL,
                initial_total BIGINT NOT NULL CHECK (initial_total >= 0),
                started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed BOOLEAN NOT NULL DEFAULT FALSE,
                finished_at TIMESTAMPTZ
            );
            CREATE INDEX flashcard_runs_user_started ON flashcard_runs(user_id, started_at DESC);
            ALTER TABLE flashcard_attempts ADD COLUMN run_id UUID REFERENCES flashcard_runs(id) ON DELETE SET NULL;
            CREATE INDEX flashcard_attempts_run ON flashcard_attempts(run_id, reviewed_at DESC);
        "#).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE flashcard_attempts DROP COLUMN run_id; DROP TABLE flashcard_runs;",
            )
            .await?;
        Ok(())
    }
}
