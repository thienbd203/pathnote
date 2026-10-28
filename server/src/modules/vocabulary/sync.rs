use crate::{
    entities::vocabulary_cards as cards,
    modules::notes::document::{extract, ExtractedSense},
    AppError,
};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
pub fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn word_key(word: &str) -> String {
    format!("{:x}", Sha256::digest(normalize(word)))
}
fn unchanged(old: &cards::Model, new: &ExtractedSense) -> bool {
    old.active
        && old.source_entry_id == new.entry_id
        && old.word == new.word
        && old.locale == new.locale
        && old.ipa == new.ipa
        && old.ipa_uk == new.ipa_uk
        && old.ipa_us == new.ipa_us
        && old.part_of_speech == new.part_of_speech
        && old.meaning == new.meaning
        && old.topics == new.topics
        && old.examples == new.examples
}
pub(in crate::modules) struct PreparedSense {
    sense: ExtractedSense,
    word_key: String,
}
pub(in crate::modules) fn prepare(document: &Value) -> Result<Vec<PreparedSense>, AppError> {
    Ok(extract(document)?
        .into_iter()
        .map(|sense| {
            let word_key = word_key(&sense.word);
            PreparedSense { sense, word_key }
        })
        .collect())
}
/// Only compare the prepared values to current DB rows and apply the delta under the parent lock.
pub(in crate::modules) async fn sync<C: ConnectionTrait>(
    db: &C,
    session: i64,
    senses: Vec<PreparedSense>,
) -> Result<(), AppError> {
    let old: HashMap<_, _> = cards::Entity::find()
        .filter(cards::Column::SessionId.eq(session))
        .all(db)
        .await?
        .into_iter()
        .map(|c| (c.source_sense_id.clone(), c))
        .collect();
    let ids: Vec<_> = senses.iter().map(|c| c.sense.sense_id.clone()).collect();
    let now = chrono::Utc::now().fixed_offset();
    let mut rows = Vec::new();
    for PreparedSense { sense, word_key } in senses {
        let previous = old.get(&sense.sense_id);
        if previous.is_some_and(|p| unchanged(p, &sense)) {
            continue;
        }
        rows.push(cards::ActiveModel {
            session_id: Set(session),
            source_entry_id: Set(sense.entry_id),
            source_sense_id: Set(sense.sense_id),
            word: Set(sense.word),
            word_key: Set(word_key),
            locale: Set(sense.locale),
            ipa: Set(sense.ipa),
            ipa_uk: Set(sense.ipa_uk),
            ipa_us: Set(sense.ipa_us),
            part_of_speech: Set(sense.part_of_speech),
            meaning: Set(sense.meaning),
            topics: Set(sense.topics),
            examples: Set(sense.examples),
            active: Set(true),
            created_at: Set(previous.map_or(now, |p| p.created_at)),
            updated_at: Set(now),
            ..Default::default()
        });
    }
    for chunk in rows.chunks(100) {
        cards::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([cards::Column::SessionId, cards::Column::SourceSenseId])
                    .update_columns([
                        cards::Column::SourceEntryId,
                        cards::Column::Word,
                        cards::Column::WordKey,
                        cards::Column::Locale,
                        cards::Column::Ipa,
                        cards::Column::IpaUk,
                        cards::Column::IpaUs,
                        cards::Column::PartOfSpeech,
                        cards::Column::Meaning,
                        cards::Column::Topics,
                        cards::Column::Examples,
                        cards::Column::Active,
                        cards::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }
    let mut removed = cards::Entity::update_many()
        .col_expr(cards::Column::Active, Expr::value(false))
        .col_expr(cards::Column::UpdatedAt, Expr::value(now))
        .filter(cards::Column::SessionId.eq(session))
        .filter(cards::Column::Active.eq(true));
    if !ids.is_empty() {
        removed = removed.filter(cards::Column::SourceSenseId.is_not_in(ids));
    }
    removed.exec(db).await?;
    Ok(())
}
