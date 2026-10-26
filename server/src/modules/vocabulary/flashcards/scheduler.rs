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
    if !retention.is_finite()
        || !(0.7..=0.97).contains(&retention)
        || memory.is_some_and(|m| {
            !m.stability.is_finite() || m.stability <= 0.0 || !m.difficulty.is_finite()
        })
    {
        return Err(invalid_state());
    }
    let state = memory.map(|m| MemoryState {
        stability: m.stability,
        difficulty: m.difficulty,
    });
    let elapsed = memory.map_or(0, |m| (now - m.reviewed_at).num_days().max(0) as u32);
    let next = fsrs
        .next_states(state, retention, elapsed)
        .map_err(|_| AppError::Internal("Không tính được lịch FSRS.".into()))?;
    let step = memory.map_or(1, |m| m.step);
    [next.again, next.hard, next.good, next.easy]
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            if !result.interval.is_finite()
                || !result.memory.stability.is_finite()
                || result.memory.stability <= 0.0
                || !result.memory.difficulty.is_finite()
            {
                return Err(invalid_state());
            }
            let rating = index as u8 + 1;
            // Anki-style 1m/10m learning and a single 10m relearning step.
            let short = match (step, rating) {
                (1, 1) | (2, 1) => Some((60, 1)),
                (1, 2) => Some((330, 1)),
                (1, 3) | (2, 2) => Some((600, 2)),
                (0, 1) => Some((600, 2)),
                _ => None,
            };
            let (seconds, step) = match short {
                Some(short) => short,
                None => (long_term_seconds(result.interval)?, 0),
            };
            Ok(Outcome {
                rating,
                seconds,
                due_at: now
                    .checked_add_signed(Duration::seconds(seconds))
                    .ok_or_else(invalid_state)?,
                stability: result.memory.stability,
                difficulty: result.memory.difficulty,
                step,
            })
        })
        .collect()
}

fn invalid_state() -> AppError {
    AppError::Internal("Trạng thái FSRS không hợp lệ.".into())
}
fn long_term_seconds(interval: f32) -> Result<i64, AppError> {
    if !interval.is_finite() {
        return Err(invalid_state());
    }
    Ok(interval.round().clamp(1.0, 36500.0) as i64 * 86400)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_finite_intervals_and_retention_never_become_immediately_due_cards() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(long_term_seconds(value).is_err());
            assert!(outcomes(
                &FSRS::new(&fsrs::DEFAULT_PARAMETERS).unwrap(),
                None,
                value,
                chrono::Utc::now().fixed_offset()
            )
            .is_err());
        }
        assert_eq!(long_term_seconds(0.0).unwrap(), 86400);
        assert_eq!(long_term_seconds(1.5).unwrap(), 2 * 86400);
        assert_eq!(long_term_seconds(50000.0).unwrap(), 36500 * 86400);
    }
}
