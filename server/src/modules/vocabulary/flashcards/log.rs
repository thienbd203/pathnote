use super::{dto, queue};
use crate::{
    access_token::CurrentUser,
    entities::{flashcard_attempts as attempts, places, sessions, vocabulary_cards as cards},
    modules::notes::service as notes,
    pagination::{ListQuery, Page, PageMeta},
    state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::QueryRejection, Query, State},
    Json,
};
use chrono::{NaiveDate, TimeZone, Utc};
use sea_orm::{
    sea_query::{extension::postgres::PgExpr, Expr, LikeExpr},
    ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, RelationTrait,
};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    session_id: Option<i64>,
    course_id: Option<i64>,
    topic: Option<String>,
    q: Option<String>,
    date: Option<NaiveDate>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    run_id: Option<uuid::Uuid>,
    card_id: Option<i64>,
    timezone: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
    rating: Option<i32>,
}
pub async fn list(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Query<Filters>, QueryRejection>,
) -> Result<Json<Page<Value>>, AppError> {
    let Query(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc lịch sử không hợp lệ."))?;
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
    queue::authorize(s.db.as_ref(), user.id, &filters).await?;
    let mut accessible = notes::accessible(user.id, false);
    if let Some(course) = filters.course_id {
        accessible = accessible.filter(sessions::Column::CourseId.eq(course));
    }
    let ids = accessible
        .select_only()
        .column(sessions::Column::Id)
        .into_query();
    let mut query = attempts::Entity::find()
        .filter(attempts::Column::UserId.eq(user.id))
        .filter(attempts::Column::Rating.is_not_null())
        .filter(attempts::Column::ReviewedAt.is_not_null())
        .join(JoinType::InnerJoin, attempts::Relation::Card.def())
        .filter(cards::Column::SessionId.in_subquery(ids));
    if let Some(session) = filters.session_id {
        query = query.filter(cards::Column::SessionId.eq(session));
    }
    if let Some(id) = input.run_id {
        super::runs::load(s.db.as_ref(), user.id, id).await?;
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
        let pattern = format!(
            "%{}%",
            q.to_lowercase()
                .replace('!', "!!")
                .replace('%', "!%")
                .replace('_', "!_")
        );
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
    let total = query.clone().count(s.db.as_ref()).await?;
    let rows = query
        .order_by_desc(attempts::Column::ReviewedAt)
        .order_by_desc(attempts::Column::Id)
        .select_also(cards::Entity)
        .paginate(s.db.as_ref(), size)
        .fetch_page(paging.page - 1)
        .await?;
    let sources: std::collections::HashMap<_, _> = notes::accessible(user.id, false)
        .filter(
            sessions::Column::Id.is_in(
                rows.iter()
                    .filter_map(|(_, c)| c.as_ref().map(|c| c.session_id)),
            ),
        )
        .select_also(places::Entity)
        .all(s.db.as_ref())
        .await?
        .into_iter()
        .map(|(session, place)| (session.id, (session, place)))
        .collect();
    let data=rows.into_iter().filter_map(|(attempt,card)| {
        let card=card?;let (session,place)=sources.get(&card.session_id)?;
        let snapshot=attempt.snapshot.is_some();
        let content=attempt.snapshot.or_else(||(queue::key(&card)==attempt.study_key).then(||json!({"word":card.word,"meaning":card.meaning,"ipa":card.ipa,"locale":card.locale,"part_of_speech":card.part_of_speech,"topics":card.topics,"examples":card.examples})));
        Some(json!({"id":attempt.id,"card_id":card.id,"run_id":attempt.run_id,"direction":attempt.direction,"rating":attempt.rating,"reviewed_at":attempt.reviewed_at,"due_at":attempt.due_at,"snapshot":snapshot,"card":content,"source":{"id":session.id,"title":session.title,"shared":place.as_ref().is_some_and(|p|p.user_id!=user.id)}}))
    }).collect();
    Ok(Json(Page {
        data,
        meta: PageMeta {
            current_page: paging.page,
            last_page: total.div_ceil(size).max(1),
            per_page: size,
            total,
        },
    }))
}
