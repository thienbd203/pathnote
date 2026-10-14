use crate::{entities::flashcard_memory, AppError};
use chrono::{DateTime, Duration, FixedOffset};
use fsrs::{MemoryState, FSRS};
use serde::Serialize;
#[derive(Serialize)]
pub struct Outcome {
    pub rating: u8,
    pub seconds: i64,
    pub due_at: DateTime<FixedOffset>,
    pub stability: f32,
    pub difficulty: f32,
    pub step: i32,
}
/// FSRS owns the memory model and long-term intervals. Learning steps are application policy.
pub fn outcomes(
    fsrs: &FSRS,
    memory: Option<&flashcard_memory::Model>,
    retention: f32,
    now: DateTime<FixedOffset>,
) -> Result<Vec<Outcome>, AppError> {
    let state = memory.map(|m| MemoryState {
        stability: m.stability,
        difficulty: m.difficulty,
    });
    let elapsed = memory.map_or(0, |m| (now - m.reviewed_at).num_days().max(0) as u32);
    let next = fsrs
        .next_states(state, retention, elapsed)
        .map_err(|_| AppError::Internal("Không tính được lịch FSRS.".into()))?;
    let step = memory.map_or(1, |m| m.step);
    Ok([next.again, next.hard, next.good, next.easy]
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let rating = index as u8 + 1;
            // Anki-style 1m/10m learning and a single 10m relearning step.
            let short = match (step, rating) {
                (1, 1) | (2, 1) => Some((60, 1)),
                (1, 2) => Some((330, 1)),
                (1, 3) | (2, 2) => Some((600, 2)),
                (0, 1) => Some((600, 2)),
                _ => None,
            };
            let (seconds, step) = short.unwrap_or_else(|| {
                (
                    (result.interval.round().clamp(1.0, 36500.0) as i64) * 86400,
                    0,
                )
            });
            Outcome {
                rating,
                seconds,
                due_at: now + Duration::seconds(seconds),
                stability: result.memory.stability,
                difficulty: result.memory.difficulty,
                step,
            }
        })
        .collect())
}
