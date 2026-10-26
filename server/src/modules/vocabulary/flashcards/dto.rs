use crate::modules::vocabulary::{MAX_QUERY_CHARS, MAX_TOPIC_CHARS};
use crate::AppError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    #[serde(default)]
    pub new_only: bool,
    pub run_id: Option<uuid::Uuid>,
    pub topic: Option<String>,
    pub q: Option<String>,
}
impl Filters {
    pub fn validate(&mut self) -> Result<(), AppError> {
        if self.course_id.is_some_and(|id| id <= 0)
            || self.session_id.is_some_and(|id| id <= 0)
            || self
                .topic
                .as_ref()
                .is_some_and(|s| s.chars().count() > MAX_TOPIC_CHARS)
            || self
                .q
                .as_ref()
                .is_some_and(|s| s.chars().count() > MAX_QUERY_CHARS)
        {
            return Err(AppError::validation("filters", "Bộ lọc không hợp lệ."));
        }
        self.topic = self
            .topic
            .take()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        self.q = self
            .q
            .take()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        Ok(())
    }
    // Searching narrows the deck, but does not create another saved configuration.
    pub fn scope(&self) -> String {
        if let Some(course) = self.course_id {
            return crate::fingerprint::json_hash(&(
                "course",
                course,
                self.session_id,
                &self.topic,
            ))
            .expect("String serialization");
        }
        crate::fingerprint::json_hash(&(self.session_id, &self.topic))
            .expect("String serialization")
    }
}
#[derive(Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    ViEn,
    EnVi,
    Both,
}
impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Self::ViEn => "vi_en",
            Self::EnVi => "en_vi",
            Self::Both => "both",
        }
    }
    pub fn directions(self) -> &'static [&'static str] {
        match self {
            Self::ViEn => &["vi_en"],
            Self::EnVi => &["en_vi"],
            Self::Both => &["vi_en", "en_vi"],
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsInput {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    pub topic: Option<String>,
    pub q: Option<String>,
    pub direction: Direction,
    pub retention: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grade {
    pub rating: u8,
}

use chrono::NaiveDate;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryFilters {
    pub session_id: Option<i64>,
    pub course_id: Option<i64>,
    pub topic: Option<String>,
    pub q: Option<String>,
    pub date: Option<NaiveDate>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub run_id: Option<uuid::Uuid>,
    pub card_id: Option<i64>,
    pub timezone: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub rating: Option<i32>,
}

use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub direction: Direction,
    pub retention: f32,
}
impl Configuration {
    pub fn from_saved(value: Value) -> Result<Self, AppError> {
        let config: Self = serde_json::from_value(value)
            .map_err(|_| AppError::Internal("Invalid saved flashcard settings".into()))?;
        if !config.retention.is_finite() || !(0.7..=0.97).contains(&config.retention) {
            return Err(AppError::Internal(
                "Invalid saved flashcard retention".into(),
            ));
        }
        Ok(config)
    }
}
#[derive(Serialize)]
pub struct MemoryProgress {
    pub remembered: f64,
    pub forgotten: f64,
    pub unseen: u64,
    pub total: u64,
    pub as_of: DateTime<FixedOffset>,
}
#[derive(Serialize)]
pub struct Overview {
    pub word_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_progress: Option<MemoryProgress>,
    pub reviewable: u64,
    pub estimated_seconds: Option<u64>,
    pub settings: Configuration,
    pub new: u64,
    pub due: u64,
    pub learning: u64,
    pub next_due_at: Option<DateTime<FixedOffset>>,
    pub algorithm: &'static str,
}
#[derive(Serialize)]
pub struct Source {
    pub id: i64,
    pub title: String,
    pub shared: bool,
}
#[derive(Serialize)]
pub struct Audio {
    pub text: String,
    pub locale: String,
}
#[derive(Serialize)]
pub struct Front {
    pub id: Uuid,
    pub card_id: i64,
    pub direction: String,
    pub front: String,
    pub source: Source,
    pub topics: Value,
    pub part_of_speech: String,
    pub new: bool,
    pub audio: Option<Audio>,
}
#[derive(Serialize)]
pub struct Next {
    pub run: Option<RunSummary>,
    pub card: Option<Front>,
    pub overview: Overview,
}
#[derive(Serialize)]
pub struct Back {
    pub word: String,
    pub meaning: String,
    pub ipa: String,
    pub locale: String,
    pub part_of_speech: String,
    pub examples: Value,
}
#[derive(Serialize)]
pub struct Reveal {
    pub back: Back,
    pub options: Vec<super::scheduler::Outcome>,
}
#[derive(Serialize)]
pub struct GradeResult {
    pub run: Option<RunSummary>,
    pub rating: i32,
    pub due_at: Option<DateTime<FixedOffset>>,
}
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub word: String,
    pub meaning: String,
    pub ipa: String,
    pub locale: String,
    pub part_of_speech: String,
    pub topics: Value,
    pub examples: Value,
}
#[derive(Serialize)]
pub struct RunSummary {
    pub id: Uuid,
    pub filters: Filters,
    pub settings: Configuration,
    pub initial_total: i64,
    pub reviewed: u64,
    pub unique_reviewed: i64,
    pub ratings: Vec<u64>,
    pub started_at: DateTime<FixedOffset>,
    pub completed: bool,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub elapsed_seconds: i64,
}
#[derive(Serialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub card_id: i64,
    pub run_id: Option<Uuid>,
    pub direction: String,
    pub rating: Option<i32>,
    pub reviewed_at: Option<DateTime<FixedOffset>>,
    pub due_at: Option<DateTime<FixedOffset>>,
    pub snapshot: bool,
    pub card: Option<Snapshot>,
    pub source: Source,
}

#[derive(Serialize)]
pub struct Optimization {
    pub automatic: bool,
    pub state: &'static str,
    pub personalized: bool,
    pub version: i32,
    pub reviews: usize,
    pub samples: usize,
    pub minimum_samples: usize,
    pub eligible: bool,
    pub trained_reviews: i32,
    pub optimized_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatisticsPeriod {
    #[default]
    Week,
    Month,
    Year,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsInput {
    #[serde(default)]
    pub period: StatisticsPeriod,
    pub date: Option<NaiveDate>,
    pub timezone: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyTimeSegment {
    pub id: Uuid,
    pub started_at: DateTime<FixedOffset>,
    pub ended_at: DateTime<FixedOffset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyTimeInput {
    pub segments: Vec<StudyTimeSegment>,
}
#[derive(Serialize)]
pub struct StudyTimeRecorded {
    pub recorded: bool,
}
#[derive(Serialize)]
pub struct StudyTimeBucket {
    pub date: NaiveDate,
    pub seconds: f64,
}
#[derive(Serialize)]
pub struct StudyStatistics {
    pub performance: ReviewPerformance,
    pub period: StatisticsPeriod,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub today: NaiveDate,
    pub today_seconds: f64,
    pub total_seconds: f64,
    pub buckets: Vec<StudyTimeBucket>,
    pub as_of: DateTime<FixedOffset>,
}

#[derive(Serialize)]
pub struct ReviewPerformanceBucket {
    pub date: NaiveDate,
    /// Rating order: Again, Hard, Good, Easy.
    pub ratings: [u64; 4],
}
#[derive(Serialize)]
pub struct ReviewPerformance {
    pub total: u64,
    pub ratings: [u64; 4],
    /// None when no graded attempts exist; otherwise a fraction in 0..=1.
    pub remembered_rate: Option<f64>,
    pub buckets: Vec<ReviewPerformanceBucket>,
}
