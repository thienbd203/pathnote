use super::super::{dto, queue};
use crate::{
    entities::{flashcard_attempts as attempts, places, sessions, vocabulary_cards as cards},
    modules::notes::service as notes,
    pagination::{ListQuery, Page, PageMeta},
    AppError,
};
use chrono::{TimeZone, Utc};
use sea_orm::{
    sea_query::{extension::postgres::PgExpr, Expr, LikeExpr},
    ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, RelationTrait,
};
use serde_json::json;
pub async fn history(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: dto::HistoryFilters,
) -> Result<Page<dto::HistoryEntry>, AppError> {
    let mut filters = dto::Filters {
        session_id: input.session_id,
        course_id: input.course_id,
        topic: input.topic,
        q: input.q,
        ..Default::default()
    };
    filters.validate()?;
    let paging = ListQuery {
        page: input.page.unwrap_or(1),
        per_page: input.per_page,
        q: None,
    };
    let size = paging.page_size()?;
    queue::authorize(db, user, &filters).await?;
    let mut accessible = notes::accessible(user, false);
    if let Some(course) = filters.course_id {
        accessible = accessible.filter(sessions::Column::CourseId.eq(course));
    }
    let ids = accessible
        .select_only()
        .column(sessions::Column::Id)
        .into_query();
    let mut query = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(user))
        .filter(attempts::Column::Rating.is_not_null())
        .filter(attempts::Column::ReviewedAt.is_not_null())
        .join(JoinType::InnerJoin, attempts::Relation::Card.def())
        .filter(cards::Column::SessionId.in_subquery(ids));
    if let Some(session) = filters.session_id {
        query = query.filter(cards::Column::SessionId.eq(session));
    }
    if let Some(id) = input.run_id {
        super::runs::load(db, user, id).await?;
        query = query.filter(attempts::Column::RunId.eq(id));
    }
    if let Some(id) = input.card_id {
        if id <= 0 {
            return Err(AppError::validation("card_id", "Thẻ không hợp lệ."));
        }
        query = query.filter(attempts::Column::CardId.eq(id));
    }
    if let Some(rating) = input.rating {
        if !(1..=4).contains(&rating) {
            return Err(AppError::validation("rating", "Mức ghi nhớ không hợp lệ."));
        }
        query = query.filter(attempts::Column::Rating.eq(rating));
    }
    if let Some(topic) = filters.topic {
        query = query.filter(
            Expr::cust("COALESCE(flashcard_attempts.snapshot->'topics',vocabulary_cards.topics)")
                .contains(Expr::value(json!([topic]))),
        );
    }
    if let Some(q) = filters.q {
        let pattern = crate::pagination::like_contains(&q);
        query=query.filter(Condition::any().add(Expr::expr(Expr::cust("LOWER(COALESCE(flashcard_attempts.snapshot->>'word',vocabulary_cards.word))")).like(LikeExpr::new(&pattern).escape('!'))).add(Expr::expr(Expr::cust("LOWER(COALESCE(flashcard_attempts.snapshot->>'meaning',vocabulary_cards.meaning))")).like(LikeExpr::new(&pattern).escape('!'))));
    }
    let timezone: chrono_tz::Tz = input
        .timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| AppError::validation("timezone", "Múi giờ không hợp lệ."))?;
    if input.date.is_some() && (input.from.is_some() || input.to.is_some())
        || input.from.is_some() != input.to.is_some()
        || input.from.zip(input.to).is_some_and(|(from, to)| from > to)
    {
        return Err(AppError::validation("date", "Khoảng ngày không hợp lệ."));
    }
    if let Some(day) = input.date.or(input.from) {
        let next = input
            .date
            .or(input.to)
            .unwrap_or(day)
            .succ_opt()
            .ok_or_else(|| AppError::validation("date", "Ngày không hợp lệ."))?;
        let start = timezone
            .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .ok_or_else(|| {
                AppError::validation("date", "Ngày này không có mốc bắt đầu duy nhất.")
            })?;
        let end = timezone
            .from_local_datetime(&next.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .ok_or_else(|| {
                AppError::validation("date", "Ngày kế tiếp không có mốc bắt đầu duy nhất.")
            })?;
        query = query
            .filter(attempts::Column::ReviewedAt.gte(start.with_timezone(&Utc).fixed_offset()))
            .filter(attempts::Column::ReviewedAt.lt(end.with_timezone(&Utc).fixed_offset()));
    }
    let total = query.clone().count(db).await?;
    let rows = query
        .order_by_desc(attempts::Column::ReviewedAt)
        .order_by_desc(attempts::Column::Id)
        .select_also(cards::Entity)
        .paginate(db, size)
        .fetch_page(paging.page - 1)
        .await?;
    let sources: std::collections::HashMap<_, _> = notes::accessible(user, false)
        .filter(
            sessions::Column::Id.is_in(
                rows.iter()
                    .filter_map(|(_, c)| c.as_ref().map(|c| c.session_id)),
            ),
        )
        .select_also(places::Entity)
        .all(db)
        .await?
        .into_iter()
        .map(|(session, place)| (session.id, (session, place)))
        .collect();
    let data = rows
        .into_iter()
        .filter_map(|(attempt, card)| {
            let card = card?;
            let (session, place) = sources.get(&card.session_id)?;
            let snapshot = attempt.snapshot.is_some();
            let content = match attempt.snapshot {
                Some(value) => Some(value),
                None if queue::key(&card) == attempt.study_key => Some(
                    serde_json::to_value(dto::Snapshot {
                        word: card.word,
                        meaning: card.meaning,
                        ipa: card.ipa,
                        locale: card.locale,
                        part_of_speech: card.part_of_speech,
                        topics: card.topics,
                        examples: card.examples,
                    })
                    .expect("Snapshot serialization"),
                ),
                None => None,
            };
            Some(dto::HistoryEntry {
                id: attempt.id,
                card_id: card.id,
                run_id: attempt.run_id,
                direction: attempt.direction,
                rating: attempt.rating,
                reviewed_at: attempt.reviewed_at,
                due_at: attempt.due_at,
                snapshot,
                card: content,
                source: dto::Source {
                    id: session.id,
                    title: session.title.clone(),
                    shared: place.as_ref().is_some_and(|p| p.user_id != user),
                },
            })
        })
        .collect();
    Ok(Page {
        data,
        meta: PageMeta {
            current_page: paging.page,
            last_page: total.div_ceil(size).max(1),
            per_page: size,
            total,
        },
    })
}
