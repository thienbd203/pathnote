use super::dto::*;
use crate::{config::Config, AppError};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, time::Duration};
pub struct LearningServices {
    http: reqwest::Client,
    pronunciation: Option<String>,
    translation: Option<String>,
}
fn unavailable() -> AppError {
    AppError::Unavailable("Dịch vụ học tập chưa sẵn sàng. Bạn có thể nhập tay hoặc thử lại.".into())
}
fn valid(s: &str) -> bool {
    !s.trim().is_empty()
        && s.chars().count() <= 6000
        && !s
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
impl LearningServices {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()?,
            pronunciation: config.pronunciation_url.clone(),
            translation: config.translation_url.clone(),
        })
    }
    pub fn enabled(&self) -> bool {
        self.pronunciation.is_some() && self.translation.is_some()
    }
    async fn post<B: Serialize, R: DeserializeOwned>(
        &self,
        base: &Option<String>,
        path: &str,
        body: B,
        timeout: u64,
    ) -> Result<R, AppError> {
        let base = base.as_ref().ok_or_else(unavailable)?;
        let mut response = self
            .http
            .post(format!("{base}{path}"))
            .timeout(Duration::from_secs(timeout))
            .json(&body)
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success() {
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len() + chunk.len() > 512 * 1024 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| unavailable())
    }

    pub async fn ipa(&self, text: &str, locale: &str) -> Result<IpaResource, AppError> {
        let result: UpstreamIpa = self
            .post(
                &self.pronunciation,
                "/ipa",
                Ipa {
                    text: text.into(),
                    locale: locale.into(),
                },
                6,
            )
            .await?;
        if result.locale != locale || !valid(&result.ipa) {
            return Err(unavailable());
        }
        Ok(IpaResource {
            ipa: result.ipa,
            locale: locale.into(),
            engine: "espeak-ng",
        })
    }
    pub async fn batch(&self, texts: &[String], locale: &str) -> Result<BatchResource, AppError> {
        let mut unique = Vec::new();
        for text in texts {
            if !unique.contains(text) {
                unique.push(text.clone());
            }
        }
        let result: UpstreamBatch = self
            .post(
                &self.pronunciation,
                "/ipa/batch",
                Batch {
                    texts: unique.clone(),
                    locale: locale.into(),
                },
                15,
            )
            .await?;
        if result.locale != locale || result.results.len() != unique.len() {
            return Err(unavailable());
        }
        let mut answers = HashMap::new();
        for (row, text) in result.results.into_iter().zip(&unique) {
            if row.text != *text {
                return Err(unavailable());
            }
            let answer = match row.ipa {
                Some(ipa) if valid(&ipa) => BatchItemResource::Success {
                    text: text.clone(),
                    ipa,
                },
                None if row.error.as_deref() == Some("generation_failed") => {
                    BatchItemResource::Error {
                        text: text.clone(),
                        error: "Chưa tạo được IPA. Bạn hãy thử lại.".into(),
                    }
                }
                _ => return Err(unavailable()),
            };
            answers.insert(text, answer);
        }
        Ok(BatchResource {
            locale: locale.into(),
            engine: "espeak-ng",
            results: texts.iter().map(|text| answers[text].clone()).collect(),
        })
    }
    pub async fn translate(&self, text: &str) -> Result<TranslationResource, AppError> {
        let result: UpstreamTranslation = self
            .post(
                &self.translation,
                "/translate",
                TranslationRequest {
                    q: text,
                    source: "en",
                    target: "vi",
                    format: "text",
                },
                30,
            )
            .await?;
        if !valid(&result.translated_text) {
            return Err(unavailable());
        }
        Ok(TranslationResource {
            meaning: result.translated_text,
            source: "en",
            target: "vi",
            engine: "libretranslate",
        })
    }
}
