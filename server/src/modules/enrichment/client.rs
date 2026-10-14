use crate::{config::Config, AppError};
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};
pub struct LearningServices {
    http: reqwest::Client,
    pronunciation: Option<String>,
    translation: Option<String>,
}
fn unavailable() -> AppError {
    AppError::Unavailable("Dịch vụ học tập chưa sẵn sàng. Bạn có thể nhập tay hoặc thử lại.".into())
}
fn valid(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        !s.trim().is_empty()
            && s.chars().count() <= 6000
            && !s
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    })
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
    async fn post(
        &self,
        base: &Option<String>,
        path: &str,
        body: Value,
        timeout: u64,
    ) -> Result<Value, AppError> {
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
    pub async fn ipa(&self, text: &str, locale: &str) -> Result<Value, AppError> {
        let result = self
            .post(
                &self.pronunciation,
                "/ipa",
                json!({"text":text,"locale":locale}),
                6,
            )
            .await?;
        if result["locale"] != locale || !valid(&result["ipa"]) {
            return Err(unavailable());
        }
        Ok(json!({"ipa":result["ipa"],"locale":locale,"engine":"espeak-ng"}))
    }
    pub async fn batch(&self, texts: &[String], locale: &str) -> Result<Value, AppError> {
        let mut unique = Vec::new();
        for text in texts {
            if !unique.contains(text) {
                unique.push(text.clone());
            }
        }
        let result = self
            .post(
                &self.pronunciation,
                "/ipa/batch",
                json!({"texts":unique,"locale":locale}),
                15,
            )
            .await?;
        let rows = result["results"].as_array().ok_or_else(unavailable)?;
        if result["locale"] != locale || rows.len() != unique.len() {
            return Err(unavailable());
        }
        let mut answers = HashMap::new();
        for (row, text) in rows.iter().zip(&unique) {
            if row["text"] != *text {
                return Err(unavailable());
            }
            let answer = if valid(&row["ipa"]) {
                json!({"text":text,"ipa":row["ipa"]})
            } else if row.get("ipa").is_none() && row["error"] == "generation_failed" {
                json!({"text":text,"error":"Chưa tạo được IPA. Bạn hãy thử lại."})
            } else {
                return Err(unavailable());
            };
            answers.insert(text, answer);
        }
        Ok(
            json!({"locale":locale,"engine":"espeak-ng","results":texts.iter().map(|t| answers[t].clone()).collect::<Vec<_>>()}),
        )
    }
    pub async fn translate(&self, text: &str) -> Result<Value, AppError> {
        let result = self
            .post(
                &self.translation,
                "/translate",
                json!({"q":text,"source":"en","target":"vi","format":"text"}),
                30,
            )
            .await?;
        if !valid(&result["translatedText"]) {
            return Err(unavailable());
        }
        Ok(
            json!({"meaning":result["translatedText"],"source":"en","target":"vi","engine":"libretranslate"}),
        )
    }
}
