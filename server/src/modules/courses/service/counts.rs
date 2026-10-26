// Shared expressions keep single-course and bulk summaries on the same block-counting rules.
pub(super) const WORDS: &str = r#"(SELECT COUNT(DISTINCT node #>> '{attrs,id}')
             FROM jsonb_path_query(n.content, '$.** ? (@.type == "vocabularyEntry")') AS node
             WHERE btrim(COALESCE(node #>> '{attrs,word}', '')) <> '')"#;
pub(super) const EXERCISES: &str = r#"(SELECT COUNT(DISTINCT node #>> '{attrs,exerciseId}')
             FROM jsonb_path_query(n.content, '$.** ? (@.type == "exercise")') AS node)"#;
