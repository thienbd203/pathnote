use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Cards::Table)
                    .add_column(ColumnDef::new(Cards::IpaUk).text().not_null().default(""))
                    .to_owned(),
            )
            .await?;
        // PostgreSQL recursive JSON traversal backfills the derived projection only.
        // SeaQuery does not express this recursive/lateral JSONB transformation.
        // Never rewrite notes/Yjs or locale: existing study keys and FSRS histories survive.
        manager.get_connection().execute_unprepared(r#"
            WITH RECURSIVE nodes(session_id, node, locale, entry) AS (
                SELECT session_id, content, ''::text, '{}'::jsonb FROM notes
                UNION ALL
                SELECT n.session_id, child,
                    CASE WHEN child->>'type' = 'vocabulary' THEN child#>>'{attrs,locale}' ELSE n.locale END,
                    CASE WHEN child->>'type' = 'vocabularyEntry' THEN child ELSE n.entry END
                FROM nodes n CROSS JOIN LATERAL jsonb_array_elements(
                    CASE WHEN jsonb_typeof(n.node->'content') = 'array' THEN n.node->'content' ELSE '[]'::jsonb END
                ) child
            ), senses AS (
                SELECT * FROM nodes WHERE node->>'type' = 'vocabularySense'
            )
            UPDATE vocabulary_cards c SET
                ipa_uk = COALESCE(NULLIF(btrim(s.entry#>>'{attrs,ipaUK}'), ''),
                    CASE WHEN s.locale = 'en-GB' THEN btrim(s.entry#>>'{attrs,ipa}') END, ''),
                examples = COALESCE((
                    SELECT jsonb_agg(e.value || jsonb_build_object(
                        'ipaUK', COALESCE(raw.value#>>'{attrs,ipaUK}', ''),
                        'ipaUS', COALESCE(raw.value#>>'{attrs,ipaUS}', '')
                    ) ORDER BY e.ordinality)
                    FROM jsonb_array_elements(c.examples) WITH ORDINALITY e(value, ordinality)
                    LEFT JOIN LATERAL (
                        SELECT value FROM jsonb_array_elements(s.node->'content')
                        WHERE value#>>'{attrs,id}' = e.value->>'id' LIMIT 1
                    ) raw ON true
                ), c.examples)
            FROM senses s
            WHERE c.session_id = s.session_id
                AND c.source_entry_id = s.entry#>>'{attrs,id}'
                AND c.source_sense_id = s.node#>>'{attrs,id}'
        "#).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Cards::Table)
                    .drop_column(Cards::IpaUk)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Cards {
    #[sea_orm(iden = "vocabulary_cards")]
    Table,
    IpaUk,
}
