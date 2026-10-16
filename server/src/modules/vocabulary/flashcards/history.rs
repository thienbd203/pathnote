use crate::{entities::flashcard_attempts as attempts, AppError};
use chrono::{DateTime, FixedOffset};
use fsrs::{FSRSItem, FSRSReview};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::collections::BTreeMap;
pub const MIN_SAMPLES: usize = 64;
pub const MAX_REVIEWS: u64 = 50_000;
pub const MAX_SEQUENCE: usize = 256;
pub struct History {
    pub reviews: Vec<FSRSReview>,
}
pub struct Dataset {
    pub histories: Vec<History>,
}
impl Dataset {
    pub fn review_count(&self) -> usize {
        self.histories.iter().map(|h| h.reviews.len()).sum()
    }
    pub fn sample_count(&self) -> usize {
        self.histories
            .iter()
            .map(|h| {
                h.reviews
                    .iter()
                    .take(MAX_SEQUENCE)
                    .filter(|r| r.delta_t > 0)
                    .count()
            })
            .sum()
    }
    pub fn training(&self) -> (Vec<FSRSItem>, Vec<i64>) {
        let mut items = Vec::new();
        let mut ids = Vec::new();
        for (id, h) in self.histories.iter().enumerate() {
            for n in 2..=h.reviews.len().min(MAX_SEQUENCE) {
                if h.reviews[n - 1].delta_t == 0 {
                    continue;
                }
                items.push(FSRSItem {
                    reviews: h.reviews[..n].to_vec(),
                });
                ids.push(id as i64);
            }
        }
        (items, ids)
    }
}
/// A chain is only usable when it starts at review zero and has no missing attempts.
pub fn prepare(rows: Vec<attempts::Model>) -> Dataset {
    type Identity = (i64, String, String);
    type Chain = (Vec<FSRSReview>, Option<DateTime<FixedOffset>>);
    let mut chains: BTreeMap<Identity, Chain> = BTreeMap::new();
    let mut histories = Vec::new();
    for row in rows {
        let (Some(rating), Some(at)) = (row.rating, row.reviewed_at) else {
            continue;
        };
        if !(1..=4).contains(&rating) {
            continue;
        }
        let identity = (row.memory_card_id, row.direction, row.study_key);
        let chain = chains.entry(identity.clone()).or_default();
        if row.version == 0 && !chain.0.is_empty() {
            histories.push(History {
                reviews: std::mem::take(&mut chain.0),
            });
            chain.1 = None;
        }
        if row.version < 0
            || row.version as usize != chain.0.len()
            || chain.1.is_some_and(|previous| at < previous)
        {
            chain.0.clear();
            chain.1 = None;
            continue;
        }
        let delta = if row.version == 0 {
            0
        } else {
            row.delta_days.map_or_else(
                || (at - chain.1.expect("Previous review exists")).num_days(),
                i64::from,
            )
        };
        if !(0..=i64::from(u32::MAX)).contains(&delta) {
            chain.0.clear();
            chain.1 = None;
            continue;
        }
        chain.0.push(FSRSReview {
            rating: rating as u32,
            delta_t: delta as u32,
        });
        chain.1 = Some(at);
    }
    for (_, (reviews, _)) in chains {
        if !reviews.is_empty() {
            histories.push(History { reviews });
        }
    }
    Dataset { histories }
}
pub async fn load<C: ConnectionTrait>(db: &C, user: i64) -> Result<Dataset, AppError> {
    let rows = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(user))
        .filter(attempts::Column::Rating.is_not_null())
        .filter(attempts::Column::ReviewedAt.is_not_null())
        .order_by_asc(attempts::Column::ReviewedAt)
        .order_by_asc(attempts::Column::CreatedAt)
        .order_by_asc(attempts::Column::Id)
        .limit(MAX_REVIEWS + 1)
        .all(db)
        .await?;
    if rows.len() > MAX_REVIEWS as usize {
        return Err(AppError::Unavailable(
            "Lịch sử quá lớn cho lượt tối ưu hiện tại.".into(),
        ));
    }
    Ok(prepare(rows))
}
pub async fn current<C: ConnectionTrait>(
    db: &C,
    user: i64,
    memory_card: i64,
    direction: &str,
    key: &str,
    reviews: i32,
) -> Result<FSRSItem, AppError> {
    let rows = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(user))
        .filter(attempts::Column::MemoryCardId.eq(memory_card))
        .filter(attempts::Column::Direction.eq(direction))
        .filter(attempts::Column::StudyKey.eq(key))
        .filter(attempts::Column::Rating.is_not_null())
        .filter(attempts::Column::ReviewedAt.is_not_null())
        .order_by_asc(attempts::Column::ReviewedAt)
        .order_by_asc(attempts::Column::CreatedAt)
        .order_by_asc(attempts::Column::Id)
        .all(db)
        .await?;
    let start = rows
        .iter()
        .rposition(|r| r.version == 0)
        .ok_or_else(|| AppError::Conflict("Lịch sử thẻ không đầy đủ.".into()))?;
    let history = prepare(rows.into_iter().skip(start).collect())
        .histories
        .pop()
        .filter(|h| h.reviews.len() == reviews as usize)
        .ok_or_else(|| AppError::Conflict("Lịch sử thẻ không đầy đủ. Hãy lấy thẻ mới.".into()))?;
    Ok(FSRSItem {
        reviews: history.reviews,
    })
}
