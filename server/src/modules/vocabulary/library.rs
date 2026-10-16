use crate::{
    access_token::CurrentUser,
    entities::{places, sessions, vocabulary_cards as cards, vocabulary_progress as progress},
    modules::notes::service as notes,
    state::AppState,
    AppError,
};
use axum::{
    extract::{rejection::QueryRejection, Query, State},
    Json,
};
use sea_orm::{
    sea_query::{extension::postgres::PgExpr, Alias, Expr, Func},
    ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, RelationTrait,
};
use serde::Deserialize;
use serde_json::{json, Value};
fn first() -> u64 {
    1
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    session_id: Option<i64>,
    course_id: Option<i64>,
    topic: Option<String>,
    q: Option<String>,
    #[serde(default = "first")]
    page: u64,
}
pub(super) fn scope(user: i64, session: Option<i64>) -> sea_orm::Select<cards::Entity> {
    let ids = notes::accessible(user, false)
        .select_only()
        .column(sessions::Column::Id)
        .into_query();
    let mut query = cards::Entity::find()
        .filter(cards::Column::Active.eq(true))
        .filter(cards::Column::SessionId.in_subquery(ids));
    if let Some(id) = session {
        query = query.filter(cards::Column::SessionId.eq(id));
    }
    query
}
pub(super) fn within_course(
    query: sea_orm::Select<cards::Entity>,
    course: Option<i64>,
) -> sea_orm::Select<cards::Entity> {
    if let Some(id) = course {
        query.filter(
            cards::Column::SessionId.in_subquery(
                sessions::Entity::find()
                    .select_only()
                    .column(sessions::Column::Id)
                    .filter(sessions::Column::CourseId.eq(id))
                    .into_query(),
            ),
        )
    } else {
        query
    }
}
pub async fn list(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Query<Filters>, QueryRejection>,
) -> Result<Json<Value>, AppError> {
    let Query(input) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    if input.page == 0 || input.page > i32::MAX as u64 {
        return Err(AppError::validation("page", "Trang không hợp lệ."));
    }
    if input.q.as_ref().is_some_and(|q| q.chars().count() > 200)
        || input.topic.as_ref().is_some_and(|t| t.chars().count() > 60)
    {
        return Err(AppError::validation("q", "Bộ lọc quá dài."));
    }
    if let Some(id) = input.course_id {
        if id <= 0 {
            return Err(AppError::validation("course_id", "Khóa học không hợp lệ."));
        }
        crate::modules::courses::service::model(s.db.as_ref(), user.id, id).await?;
    }
    let source = if let Some(id) = input.session_id {
        if id <= 0 {
            return Err(AppError::validation("session_id", "Buổi học không hợp lệ."));
        }
        let (session, place) = notes::access(s.db.as_ref(), user.id, id, false).await?;
        Some(json!({"id":id,"title":session.title,"shared":place.user_id != user.id}))
    } else {
        None
    };
    let base = within_course(scope(user.id, input.session_id), input.course_id);
    let topics: Vec<String> = base
        .clone()
        .select_only()
        .expr_as(
            Func::cust(Alias::new("jsonb_array_elements_text"))
                .arg(Expr::col((cards::Entity, cards::Column::Topics))),
            "topic",
        )
        .distinct()
        .into_tuple()
        .all(s.db.as_ref())
        .await?;
    let mut topics = topics;
    topics.sort();
    let query = filtered(base, input.topic, input.q);
    let total = query.clone().count(s.db.as_ref()).await?;
    let reviewable = query
        .clone()
        .filter(cards::Column::Meaning.ne(""))
        .count(s.db.as_ref())
        .await?;
    let word_count: i64 = query
        .clone()
        .select_only()
        .expr_as(
            Expr::col((cards::Entity, cards::Column::WordKey)).count_distinct(),
            "count",
        )
        .into_tuple()
        .one(s.db.as_ref())
        .await?
        .unwrap_or(0);
    let rows = query
        .join(JoinType::InnerJoin, cards::Relation::Session.def())
        .select_also(sessions::Entity)
        .order_by_asc(cards::Column::Word)
        .order_by_asc(cards::Column::Id)
        .paginate(s.db.as_ref(), 50)
        .fetch_page(input.page - 1)
        .await?;
    let owners: std::collections::HashMap<_, _> = places::Entity::find()
        .filter(
            places::Column::Id.is_in(
                rows.iter()
                    .filter_map(|(_, s)| s.as_ref().map(|s| s.place_id)),
            ),
        )
        .all(s.db.as_ref())
        .await?
        .into_iter()
        .map(|p| (p.id, p.user_id))
        .collect();
    let progress: std::collections::HashMap<_, _> = progress::Entity::find()
        .filter(progress::Column::UserId.eq(user.id))
        .filter(progress::Column::CardId.is_in(rows.iter().map(|(c, _)| c.id)))
        .all(s.db.as_ref())
        .await?
        .into_iter()
        .map(|p| (p.card_id, p))
        .collect();
    let data: Vec<_> = rows.into_iter().filter_map(|(card,session)| session.map(|session| {
        let stats = progress.get(&card.id).filter(|p|p.study_key==super::reviews::state::key(&card)).map(|p|json!({"correct":p.correct,"incorrect":p.incorrect,"streak":p.streak,"due_at":p.due_at})).unwrap_or_else(||json!({"correct":0,"incorrect":0,"streak":0,"due_at":null}));
        json!({"id":card.id,"word":card.word,"word_key":card.word_key,"ipa":card.ipa,"locale":card.locale,"part_of_speech":card.part_of_speech,"meaning":card.meaning,"topics":card.topics,"examples":card.examples,"source":{"id":session.id,"title":session.title,"shared":owners.get(&session.place_id) != Some(&user.id)},"progress":stats})
    })).collect();
    Ok(Json(
        json!({"data":data,"meta":{"page":input.page,"last_page":total.div_ceil(50).max(1),"total":total,"word_count":word_count,"reviewable":reviewable,"topics":topics,"syncing":false,"source":source}}),
    ))
}

pub(super) fn filtered(
    mut query: sea_orm::Select<cards::Entity>,
    topic: Option<String>,
    text: Option<String>,
) -> sea_orm::Select<cards::Entity> {
    if let Some(topic) = topic.filter(|t| !t.trim().is_empty()) {
        query = query.filter(
            Expr::col((cards::Entity, cards::Column::Topics))
                .contains(Expr::value(json!([topic.trim()]))),
        );
    }
    if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
        let pattern = crate::pagination::like_contains(text.trim());
        query = query.filter(
            Condition::any()
                .add(
                    Expr::expr(Func::lower(Expr::col((cards::Entity, cards::Column::Word))))
                        .like(sea_orm::sea_query::LikeExpr::new(&pattern).escape('!')),
                )
                .add(
                    Expr::expr(Func::lower(Expr::col((
                        cards::Entity,
                        cards::Column::Meaning,
                    ))))
                    .like(sea_orm::sea_query::LikeExpr::new(&pattern).escape('!')),
                ),
        );
    }
    query
}
