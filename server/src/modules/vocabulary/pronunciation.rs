//! Pronunciation presentation must not change the source locale used by study identities.
use crate::entities::vocabulary_cards;
use serde_json::Value;

pub const DEFAULT_LOCALE: &str = "en-GB";

pub fn british_ipa<'a>(uk: &'a str, legacy: &'a str, source_locale: &str) -> &'a str {
    if !uk.trim().is_empty() {
        uk.trim()
    } else if source_locale == DEFAULT_LOCALE {
        legacy.trim()
    } else {
        ""
    }
}

pub fn examples(card: &vocabulary_cards::Model) -> Value {
    example_values(&card.examples, &card.locale)
}

pub fn example_values(examples: &Value, source_locale: &str) -> Value {
    let mut examples = examples.clone();
    if let Some(rows) = examples.as_array_mut() {
        for row in rows {
            let ipa = british_ipa(
                row["ipaUK"].as_str().unwrap_or_default(),
                row["ipa"].as_str().unwrap_or_default(),
                source_locale,
            )
            .to_owned();
            row["ipa"] = Value::String(ipa);
        }
    }
    examples
}

pub fn ipa(card: &vocabulary_cards::Model) -> String {
    british_ipa(&card.ipa_uk, &card.ipa, &card.locale).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn american_ipa_prefers_us_and_does_not_relabel_uk() {
        assert_eq!(american_ipa(" /US/ ", "/UK/", "en-GB"), "/US/");
        assert_eq!(american_ipa("", " /legacy US/ ", "en-US"), "/legacy US/");
        assert_eq!(american_ipa(" ", "/UK/", "en-GB"), "");
    }

    #[test]
    fn prefer_uk_and_never_relabel_legacy_us_as_british() {
        assert_eq!(british_ipa(" /UK/ ", "/US/", "en-US"), "/UK/");
        assert_eq!(british_ipa("", " /legacy UK/ ", "en-GB"), "/legacy UK/");
        assert_eq!(british_ipa(" ", "/US/", "en-US"), "");
    }
}

/// Flashcards use American pronunciation without changing source identities or other reviews.
pub const FLASHCARD_LOCALE: &str = "en-US";

pub fn american_ipa<'a>(us: &'a str, legacy: &'a str, source_locale: &str) -> &'a str {
    if !us.trim().is_empty() {
        us.trim()
    } else if source_locale == FLASHCARD_LOCALE {
        legacy.trim()
    } else {
        ""
    }
}

pub fn flashcard_ipa(card: &vocabulary_cards::Model) -> String {
    american_ipa(&card.ipa_us, &card.ipa, &card.locale).into()
}

pub fn flashcard_examples(card: &vocabulary_cards::Model) -> Value {
    let mut examples = card.examples.clone();
    if let Some(rows) = examples.as_array_mut() {
        for row in rows {
            row["ipa"] = Value::String(
                american_ipa(
                    row["ipaUS"].as_str().unwrap_or_default(),
                    row["ipa"].as_str().unwrap_or_default(),
                    &card.locale,
                )
                .to_owned(),
            );
        }
    }
    examples
}
