use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_index(
            Index::create()
                .name("vocabulary_cards_word_locale_pos")
                .table(Cards::Table)
                .col(Cards::WordKey)
                .col(Cards::Locale)
                .col(Cards::PartOfSpeech)
                .to_owned(),
        )
        .await?;
        // Duplicates the prefix of flashcard_due (user_id, direction, due_at).
        m.drop_index(Index::drop().name("flashcard_memory_identity").to_owned())
            .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_index(
            Index::create()
                .name("flashcard_memory_identity")
                .table(Memory::Table)
                .col(Memory::UserId)
                .col(Memory::Direction)
                .to_owned(),
        )
        .await?;
        m.drop_index(
            Index::drop()
                .name("vocabulary_cards_word_locale_pos")
                .to_owned(),
        )
        .await
    }
}

#[derive(DeriveIden)]
enum Cards {
    #[sea_orm(iden = "vocabulary_cards")]
    Table,
    WordKey,
    Locale,
    PartOfSpeech,
}

#[derive(DeriveIden)]
enum Memory {
    #[sea_orm(iden = "flashcard_memory")]
    Table,
    UserId,
    Direction,
}
