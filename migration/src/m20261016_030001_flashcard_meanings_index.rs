use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "CREATE INDEX vocabulary_cards_word_locale_pos ON vocabulary_cards(word_key, locale, part_of_speech); \
             DROP INDEX flashcard_memory_identity;",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX flashcard_memory_identity ON flashcard_memory(user_id, direction); \
             DROP INDEX vocabulary_cards_word_locale_pos;",
            )
            .await?;
        Ok(())
    }
}
