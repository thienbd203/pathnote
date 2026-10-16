use super::dto::invalid;
use crate::AppError;
use serde::{Deserialize, Serialize};

use std::collections::HashSet;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Question {
    Choice {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Fill {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Classify {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Matching {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
        items: Vec<String>,
    },
    Ordering {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Translation {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Correction {
        prompt: String,
        options: Vec<String>,
        answers: Vec<String>,
        explanation: String,
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
    },
}
impl Question {
    pub fn validate(&mut self) -> Result<(), AppError> {
        let (prompt, options, answers, explanation) = match self {
            Self::Choice {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Fill {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Classify {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Matching {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Ordering {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Translation {
                prompt,
                options,
                answers,
                explanation,
                ..
            }
            | Self::Correction {
                prompt,
                options,
                answers,
                explanation,
                ..
            } => (prompt, options, answers, explanation),
        };
        *prompt = prompt.trim().into();
        *explanation = explanation.trim().into();
        for s in options.iter_mut().chain(answers.iter_mut()) {
            *s = s.trim().into();
        }
        if !text(prompt, 2000)
            || explanation.chars().count() > 6000
            || answers.is_empty()
            || answers.len() > 10
            || !answers.iter().all(|s| text(s, 1000))
            || options.len() > 20
            || !options.iter().all(|s| text(s, 200))
        {
            return Err(invalid());
        }
        let valid = match self {
            Self::Choice {
                options, answers, ..
            }
            | Self::Classify {
                options, answers, ..
            } => {
                (2..=6).contains(&options.len())
                    && unique(options)
                    && answers.len() == 1
                    && options.contains(&answers[0])
            }
            Self::Fill { options, .. }
            | Self::Translation { options, .. }
            | Self::Correction { options, .. } => options.is_empty(),
            Self::Matching {
                items,
                options,
                answers,
                ..
            } => {
                for item in items.iter_mut() {
                    *item = item.trim().into();
                }
                (2..=8).contains(&items.len())
                    && items.iter().all(|s| text(s, 200))
                    && unique(items)
                    && unique(options)
                    && unique(answers)
                    && options.len() == items.len()
                    && answers.len() == items.len()
                    && answers.iter().all(|s| options.contains(s))
            }
            Self::Ordering {
                options, answers, ..
            } => {
                (2..=20).contains(&options.len())
                    && answers.len() == 1
                    && tokens(&answers[0]) == sorted(options)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid())
        }
    }
    pub fn public(&self) -> super::dto::PublicQuestionResource {
        let (kind, prompt, options) = match self {
            Self::Choice {
                prompt, options, ..
            } => ("choice", prompt, options),
            Self::Fill {
                prompt, options, ..
            } => ("fill", prompt, options),
            Self::Classify {
                prompt, options, ..
            } => ("classify", prompt, options),
            Self::Matching {
                prompt, options, ..
            } => ("matching", prompt, options),
            Self::Ordering {
                prompt, options, ..
            } => ("ordering", prompt, options),
            Self::Translation {
                prompt, options, ..
            } => ("translation", prompt, options),
            Self::Correction {
                prompt, options, ..
            } => ("correction", prompt, options),
        };
        super::dto::PublicQuestionResource {
            kind,
            prompt: prompt.clone(),
            options: options.clone(),
            items: if let Self::Matching { items, .. } = self {
                Some(items.clone())
            } else {
                None
            },
        }
    }

    pub fn answers(&self) -> &[String] {
        match self {
            Self::Choice { answers, .. }
            | Self::Fill { answers, .. }
            | Self::Classify { answers, .. }
            | Self::Matching { answers, .. }
            | Self::Ordering { answers, .. }
            | Self::Translation { answers, .. }
            | Self::Correction { answers, .. } => answers,
        }
    }
    pub fn explanation(&self) -> &str {
        match self {
            Self::Choice { explanation, .. }
            | Self::Fill { explanation, .. }
            | Self::Classify { explanation, .. }
            | Self::Matching { explanation, .. }
            | Self::Ordering { explanation, .. }
            | Self::Translation { explanation, .. }
            | Self::Correction { explanation, .. } => explanation,
        }
    }
    pub fn answer_limit(&self) -> usize {
        if matches!(self, Self::Matching { .. }) {
            4000
        } else {
            1000
        }
    }
    pub fn grade(&self, answer: &str) -> Result<bool, AppError> {
        match self {
            Self::Choice {
                options, answers, ..
            }
            | Self::Classify {
                options, answers, ..
            } => {
                if !options.iter().any(|o| o == answer) {
                    return Err(AppError::validation(
                        "answer",
                        "Hãy chọn một phương án trong đề.",
                    ));
                }
                return Ok(answers[0] == answer);
            }
            Self::Matching {
                items,
                options,
                answers,
                ..
            } => {
                let pairs: Vec<String> = serde_json::from_str(answer)
                    .map_err(|_| AppError::validation("answer", "Hãy nối đủ các cặp."))?;
                if pairs.len() != items.len()
                    || !unique(&pairs)
                    || pairs.iter().any(|s| !options.contains(s))
                {
                    return Err(AppError::validation(
                        "answer",
                        "Mỗi mục phải nối với một phương án khác nhau trong đề.",
                    ));
                }
                return Ok(&pairs == answers);
            }
            Self::Ordering { options, .. } if tokens(answer) != sorted(options) => {
                return Err(AppError::validation(
                    "answer",
                    "Câu phải dùng đúng toàn bộ các từ trong đề.",
                ))
            }
            _ => {}
        }
        let sensitive = match self {
            Self::Choice { case_sensitive, .. }
            | Self::Fill { case_sensitive, .. }
            | Self::Classify { case_sensitive, .. }
            | Self::Matching { case_sensitive, .. }
            | Self::Ordering { case_sensitive, .. }
            | Self::Translation { case_sensitive, .. }
            | Self::Correction { case_sensitive, .. } => *case_sensitive,
        };
        let normalize = |s: &str| {
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if sensitive {
                s
            } else {
                s.to_lowercase()
            }
        };
        Ok(self
            .answers()
            .iter()
            .any(|s| normalize(s) == normalize(answer)))
    }
}
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max
}
fn unique(s: &[String]) -> bool {
    s.iter().collect::<HashSet<_>>().len() == s.len()
}
fn tokens(s: &str) -> Vec<String> {
    let mut words = s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    words.sort();
    words
}
fn sorted(s: &[String]) -> Vec<String> {
    let mut words = s.to_vec();
    words.sort();
    words
}
