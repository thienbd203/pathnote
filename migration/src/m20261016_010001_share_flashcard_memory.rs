use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Raw SQL: the DO block drops foreign keys found in pg_constraint.
        // Existing duplicate memories may have different schedules. Keep the
        // most recently reviewed schedule; every attempt remains in history.
        manager.get_connection().execute_unprepared(r#"
            DELETE FROM flashcard_memory m USING (
                SELECT user_id, card_id, direction,
                    ROW_NUMBER() OVER (
                        PARTITION BY user_id, study_key, direction
                        ORDER BY reviewed_at DESC, reviews DESC, card_id DESC
                    ) AS rank
                FROM flashcard_memory
            ) duplicates
            WHERE duplicates.rank > 1 AND m.user_id = duplicates.user_id
                AND m.card_id = duplicates.card_id AND m.direction = duplicates.direction;

            ALTER TABLE flashcard_memory DROP CONSTRAINT flashcard_memory_pkey;
            ALTER TABLE flashcard_memory ADD PRIMARY KEY (user_id, study_key, direction);

            -- A source lesson can disappear while the same word is still in
            -- another lesson. Memory and training history belong to the user.
            DO $$ DECLARE item RECORD; BEGIN
                FOR item IN SELECT conname, conrelid::regclass AS relation
                    FROM pg_constraint WHERE contype = 'f'
                    AND confrelid = 'vocabulary_cards'::regclass
                    AND conrelid IN ('flashcard_memory'::regclass, 'flashcard_attempts'::regclass)
                LOOP
                    EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', item.relation, item.conname);
                END LOOP;
            END $$;
            CREATE INDEX flashcard_memory_identity ON flashcard_memory(user_id, direction);
            CREATE INDEX flashcard_attempts_identity ON flashcard_attempts(user_id, study_key, direction, reviewed_at);
        "#).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Shared flashcard schedules cannot be split back into lesson schedules without losing review state.".into()))
    }
}
