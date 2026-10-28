use super::super::dto::Snapshot;
use crate::{entities::vocabulary_cards, modules::vocabulary::pronunciation};

/// Presentation defaults are shared by new snapshots and missing-snapshot fallbacks.
/// Stored historical snapshots are immutable and never run through this conversion.
pub(super) fn snapshot(card: &vocabulary_cards::Model, meanings: String) -> Snapshot {
    Snapshot {
        word: card.word.clone(),
        meaning: meanings,
        ipa: pronunciation::flashcard_ipa(card),
        locale: pronunciation::FLASHCARD_LOCALE.into(),
        part_of_speech: card.part_of_speech.clone(),
        topics: card.topics.clone(),
        examples: pronunciation::flashcard_examples(card),
    }
}
