use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r#"
            ALTER TABLE flashcard_attempts ADD COLUMN content_key VARCHAR(64);
            ALTER TABLE flashcard_attempts ADD COLUMN memory_card_id BIGINT;
            UPDATE flashcard_attempts SET content_key = study_key, memory_card_id = card_id;
            ALTER TABLE flashcard_attempts ALTER COLUMN content_key SET NOT NULL;
            ALTER TABLE flashcard_attempts ALTER COLUMN memory_card_id SET NOT NULL;

            CREATE FUNCTION pg_temp.pathnote_word_key(word TEXT) RETURNS TEXT
            LANGUAGE SQL IMMUTABLE AS $$
                SELECT encode(sha256(convert_to(lower(btrim(regexp_replace(word, U&'[\0009-\000D\0020\0085\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]+', ' ', 'g'))), 'UTF8')), 'hex')
            $$;

            CREATE FUNCTION pg_temp.pathnote_group_key(word TEXT, locale TEXT, pos TEXT) RETURNS TEXT
            LANGUAGE SQL IMMUTABLE AS $$
                SELECT encode(sha256(convert_to(array_to_json(ARRAY[pg_temp.pathnote_word_key(word), locale, pos])::text, 'UTF8')), 'hex')
            $$;

            -- Some legacy attempts have no snapshot and their source has
            -- since been edited. Recover their identity from old memory first.
            CREATE TEMP TABLE pathnote_flashcard_group_keys ON COMMIT DROP AS
            SELECT DISTINCT user_id, study_key AS old_key,
                pg_temp.pathnote_group_key(identity->>0, identity->>2, identity->>3) AS new_key
            FROM flashcard_memory;

            -- Keep the newest state and its original history lineage when
            -- previously separate meanings now become one study group.
            DELETE FROM flashcard_memory m USING (
                SELECT user_id, study_key, direction, ROW_NUMBER() OVER (
                    PARTITION BY user_id, direction, pg_temp.pathnote_group_key(identity->>0, identity->>2, identity->>3)
                    ORDER BY reviewed_at DESC, reviews DESC, card_id DESC
                ) AS rank FROM flashcard_memory
            ) duplicates WHERE duplicates.rank > 1 AND m.user_id = duplicates.user_id
                AND m.study_key = duplicates.study_key AND m.direction = duplicates.direction;
            UPDATE flashcard_memory SET
                study_key = pg_temp.pathnote_group_key(identity->>0, identity->>2, identity->>3),
                identity = jsonb_build_array(pg_temp.pathnote_word_key(identity->>0), identity->>2, identity->>3);
            WITH identities AS (
                SELECT a.id, COALESCE(m.new_key,
                    CASE WHEN COALESCE(a.snapshot->>'word', c.word) IS NOT NULL
                        THEN pg_temp.pathnote_group_key(COALESCE(a.snapshot->>'word', c.word),
                            COALESCE(a.snapshot->>'locale', c.locale),
                            COALESCE(a.snapshot->>'part_of_speech', c.part_of_speech))
                    END, a.study_key) AS new_key
                FROM flashcard_attempts a LEFT JOIN vocabulary_cards c ON c.id = a.card_id
                LEFT JOIN pathnote_flashcard_group_keys m ON m.user_id = a.user_id AND m.old_key = a.study_key
            ) UPDATE flashcard_attempts a SET study_key = i.new_key FROM identities i WHERE a.id = i.id;
            DROP FUNCTION pg_temp.pathnote_group_key(TEXT, TEXT, TEXT);
            DROP FUNCTION pg_temp.pathnote_word_key(TEXT);
        "#).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Grouped flashcard schedules require a forward migration to preserve existing review histories.".into()))
    }
}
