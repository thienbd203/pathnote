use super::dto::{key, Pair};
use crate::modules::notes::document::Locale;
pub const SOUNDS: &[&str] = &[
    "iː", "ɪ", "ʊ", "uː", "e", "ə", "ɜː", "ɔː", "æ", "ʌ", "ɑː", "ɒ", "ɪə", "eɪ", "ʊə", "ɔɪ", "əʊ",
    "eə", "aɪ", "aʊ", "p", "b", "t", "d", "k", "ɡ", "f", "v", "θ", "ð", "s", "z", "ʃ", "ʒ", "tʃ",
    "dʒ", "m", "n", "ŋ", "h", "l", "r", "w", "j",
];
fn segments(mut ipa: &str, locale: &Locale) -> Option<Vec<String>> {
    let mut out = Vec::new();
    while !ipa.is_empty() {
        let c = ipa.chars().next()?;
        if c.is_whitespace() || "/[]ˈˌ.".contains(c) {
            ipa = &ipa[c.len_utf8()..];
            continue;
        }
        let token = SOUNDS
            .iter()
            .copied()
            .chain(["oʊ", "ɝ", "ɜ", "ɚ", "ɛ", "ɑ", "ɔ", "i", "u", "g", "ɹ"])
            .filter(|s| ipa.starts_with(s))
            .max_by_key(|s| s.chars().count())?;
        let token_out = if matches!(locale, Locale::Us) {
            match token {
                "oʊ" => "əʊ",
                "ɝ" | "ɜ" => "ɜː",
                "ɚ" => "ə",
                "ɛ" => "e",
                "ɑ" => "ɑː",
                "ɔ" => "ɔː",
                "i" => "iː",
                "u" => "uː",
                v => v,
            }
        } else {
            token
        };
        out.push(
            match token_out {
                "g" => "ɡ",
                "ɹ" => "r",
                v => v,
            }
            .into(),
        );
        ipa = &ipa[token.len()..];
    }
    Some(out)
}
pub fn minimal(pair: &Pair, sounds: &[String; 2], locale: &Locale) -> bool {
    let (Some(left), Some(right)) = (
        segments(&pair.left.ipa, locale),
        segments(&pair.right.ipa, locale),
    ) else {
        return false;
    };
    if left.len() != right.len() || key(&pair.left.word) == key(&pair.right.word) {
        return false;
    }
    let differences: Vec<_> = left.iter().zip(&right).filter(|(a, b)| a != b).collect();
    differences.len() == 1 && differences[0].0 == &sounds[0] && differences[0].1 == &sounds[1]
}
