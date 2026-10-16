use super::{
    dto::*,
    state::{key, Review},
};
use crate::{
    entities::{
        places, sessions, vocabulary_cards as cards, vocabulary_progress as progress,
        vocabulary_reviews as reviews,
    },
    modules::notes::service as notes,
    AppError,
};
use rand::seq::SliceRandom;
use sea_orm::{
    sea_query::{Alias, Expr, Func},
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use std::collections::{HashMap, HashSet};
pub async fn create(db: &DatabaseConnection, user: i64, input: Create) -> Result<Review, AppError> {
    let limit = input.limit.unwrap_or(20);
    if !(1..=50).contains(&limit)
        || input.course_id.is_some_and(|id| id <= 0)
        || input.session_id.is_some_and(|id| id <= 0)
        || input.topic.as_ref().is_some_and(|t| t.chars().count() > 60)
        || input.q.as_ref().is_some_and(|q| q.chars().count() > 200)
    {
        return Err(AppError::validation(
            "limit",
            "Bộ lọc không hợp lệ; mỗi lượt từ 1 đến 50 câu.",
        ));
    }
    if let Some(id) = input.course_id {
        crate::modules::courses::service::model(db, user, id).await?;
    }
    if let Some(id) = input.session_id {
        notes::access(db, user, id, false).await?;
    }
    let filters = Filters {
        session_id: input.session_id,
        course_id: input.course_id,
        topic: input.topic,
        q: input.q,
    };
    let pool = super::super::library::scope(user, None)
        .filter(cards::Column::Meaning.ne(""))
        .order_by_asc(Expr::expr(Func::cust(Alias::new("random"))))
        .limit(1000)
        .all(db)
        .await?;
    let mut candidates = super::super::library::filtered(
        super::super::library::within_course(
            super::super::library::scope(user, filters.session_id),
            filters.course_id,
        ),
        filters.topic.clone(),
        filters.q.clone(),
    )
    .filter(cards::Column::Meaning.ne(""))
    .order_by_asc(Expr::expr(Func::cust(Alias::new("random"))))
    .limit(1000)
    .all(db)
    .await?;
    let old: HashMap<_, _> = progress::Entity::find()
        .filter(progress::Column::UserId.eq(user))
        .filter(progress::Column::CardId.is_in(candidates.iter().map(|c| c.id)))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.card_id, p))
        .collect();
    // SQL samples a bounded random pool; incorrect words in that pool are prioritized.
    candidates.sort_by_key(|c| {
        std::cmp::Reverse(
            old.get(&c.id)
                .filter(|p| p.study_key == key(c))
                .map_or(0, |p| p.incorrect),
        )
    });
    let source_ids: HashSet<_> = candidates
        .iter()
        .chain(pool.iter())
        .map(|c| c.session_id)
        .collect();
    let sources: HashMap<_, _> = sessions::Entity::find()
        .filter(sessions::Column::Id.is_in(source_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    let owners: HashMap<_, _> = places::Entity::find()
        .filter(places::Column::Id.is_in(sources.values().map(|s| s.place_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.user_id))
        .collect();
    let mut questions = Vec::new();
    let mut seen = HashSet::new();
    {
        let mut rng = rand::thread_rng();
        for card in candidates {
            let study_key = key(&card);
            if !seen.insert(study_key.clone()) {
                continue;
            }
            let mut options = Vec::new();
            let mut source_sessions = vec![card.session_id];
            if input.mode == Mode::Choice {
                let mut words = HashSet::new();
                words.insert(card.word_key.clone());
                for other in &pool {
                    if other.locale == card.locale
                        && other.part_of_speech == card.part_of_speech
                        && super::super::sync::normalize(&other.meaning)
                            != super::super::sync::normalize(&card.meaning)
                        && words.insert(other.word_key.clone())
                    {
                        options.push(other.word.clone());
                        source_sessions.push(other.session_id);
                        if options.len() == 3 {
                            break;
                        }
                    }
                }
                if options.is_empty() {
                    continue;
                }
                options.push(card.word.clone());
                options.shuffle(&mut rng);
            }
            let Some(source) = sources.get(&card.session_id) else {
                continue;
            };
            let mut letters: Vec<_> = super::super::sync::normalize(&card.word)
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(|c| c.to_string())
                .collect();
            let original = letters.clone();
            letters.shuffle(&mut rng);
            if letters == original {
                if let Some(i) = letters.iter().position(|l| Some(l) != letters.first()) {
                    letters.swap(0, i);
                }
            }
            let examples: Vec<_> = card
                .examples
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e["text"].as_str().is_some_and(|s| !s.trim().is_empty()))
                .take(3)
                .cloned()
                .collect();
            questions.push(Question {
                card_id: card.id,
                study_key,
                source_sessions,
                card: Card {
                    word: card.word,
                    meaning: card.meaning,
                    ipa: card.ipa,
                    locale: card.locale,
                    part_of_speech: card.part_of_speech,
                    topics: card.topics,
                    examples: serde_json::Value::Array(examples),
                    source: serde_json::to_value(Source {
                        id: source.id,
                        title: source.title.clone(),
                        shared: owners.get(&source.place_id) != Some(&user),
                    })
                    .map_err(|e| AppError::Internal(e.to_string()))?,
                },
                options,
                letters,
                first_attempt: None,
            });
            if questions.len() == limit {
                break;
            }
        }
    }
    if questions.is_empty() {
        return Err(AppError::validation(
            "mode",
            if input.mode == Mode::Choice {
                "Cần ít nhất hai từ khác nhau cùng loại từ và giọng đọc, với nghĩa khác nhau. Bạn có thể chọn Gõ từ."
            } else {
                "Chưa có từ kèm nghĩa trong bộ lọc này."
            },
        ));
    }
    let now = chrono::Utc::now().fixed_offset();
    let model =
        reviews::ActiveModel {
            user_id: Set(user),
            mode: Set(input.mode.label().into()),
            filters: Set(
                serde_json::to_value(&filters).map_err(|e| AppError::Internal(e.to_string()))?
            ),
            questions: Set(
                serde_json::to_value(&questions).map_err(|e| AppError::Internal(e.to_string()))?
            ),
            answers: Set(serde_json::Value::Array(Vec::new())),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
    let review = Review::parse(model)?;
    review.authorize(db).await?;
    review.current(db).await?;
    Ok(review)
}
