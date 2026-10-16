use super::dto::*;
use crate::{
    entities::{sessions, vocabulary_cards as cards},
    modules::notes::service as notes,
    AppError,
};
use sea_orm::{
    sea_query::{extension::postgres::PgExpr, Expr, Func},
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter,
    QuerySelect, QueryTrait,
};

pub(in crate::modules::vocabulary) fn scope(
    user: i64,
    session: Option<i64>,
) -> sea_orm::Select<cards::Entity> {
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
pub(in crate::modules::vocabulary) fn within_course(
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
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: Filters,
) -> Result<Resource, AppError> {
    let per_page = crate::pagination::ListQuery {
        page: input.page,
        per_page: Some(input.per_page.unwrap_or(50)),
        q: None,
    }
    .page_size()?;
    if input.q.as_ref().is_some_and(|q| q.chars().count() > 200)
        || input.topic.as_ref().is_some_and(|t| t.chars().count() > 60)
    {
        return Err(AppError::validation("q", "Bộ lọc quá dài."));
    }
    if let Some(id) = input.course_id {
        if id <= 0 {
            return Err(AppError::validation("course_id", "Khóa học không hợp lệ."));
        }
        crate::modules::courses::service::model(db, user, id).await?;
    }
    let source = if let Some(id) = input.session_id {
        if id <= 0 {
            return Err(AppError::validation("session_id", "Buổi học không hợp lệ."));
        }
        let (session, place) = notes::access(db, user, id, false).await?;
        Some(Source {
            id,
            title: session.title,
            shared: place.user_id != user,
        })
    } else {
        None
    };
    let base = within_course(scope(user, input.session_id), input.course_id);
    let stats = statistics(db, base.clone(), input.topic.as_deref(), input.q.as_deref()).await?;
    let query = filtered(base, input.topic, input.q).build(sea_orm::DbBackend::Postgres);
    let mut values = query.values.map(|v| v.0).unwrap_or_default();
    let user_param = values.len() + 1;
    values.push(user.into());
    let limit_param = values.len() + 1;
    values.push((per_page as i64).into());
    let offset_param = values.len() + 1;
    values.push((((input.page - 1) * per_page) as i64).into());
    let rows = db.query_all(sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Postgres, format!(r#"
        SELECT c.*, s.id AS source_id, s.title AS source_title, parent.user_id <> ${user_param} AS source_shared,
          p.study_key AS progress_key, p.correct AS progress_correct, p.incorrect AS progress_incorrect, p.streak AS progress_streak, p.due_at AS progress_due_at
        FROM ({query}) c JOIN sessions s ON s.id = c.session_id JOIN places parent ON parent.id = s.place_id
        LEFT JOIN vocabulary_progress p ON p.card_id = c.id AND p.user_id = ${user_param}
        ORDER BY c.word ASC, c.id ASC LIMIT ${limit_param} OFFSET ${offset_param}
    "#, query=query.sql), values)).await?;
    let data = rows
        .into_iter()
        .map(|row| {
            let card = cards::Model::from_query_result(&row, "")?;
            let matching = row
                .try_get::<Option<String>>("", "progress_key")?
                .is_some_and(|key| key == super::super::reviews::state::key(&card));
            let progress = if matching {
                Progress {
                    correct: row.try_get("", "progress_correct")?,
                    incorrect: row.try_get("", "progress_incorrect")?,
                    streak: row.try_get("", "progress_streak")?,
                    due_at: row.try_get("", "progress_due_at")?,
                }
            } else {
                Progress::default()
            };
            Ok::<_, sea_orm::DbErr>(Card {
                id: card.id,
                word: card.word,
                word_key: card.word_key,
                ipa: card.ipa,
                locale: card.locale,
                part_of_speech: card.part_of_speech,
                meaning: card.meaning,
                topics: card.topics,
                examples: card.examples,
                source: Source {
                    id: row.try_get("", "source_id")?,
                    title: row.try_get("", "source_title")?,
                    shared: row.try_get("", "source_shared")?,
                },
                progress,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Resource {
        data,
        meta: Meta {
            page: crate::pagination::PageMeta::new(input.page, per_page, stats.total),
            word_count: stats.word_count,
            reviewable: stats.reviewable,
            topics: stats.topics,
            syncing: false,
            source,
        },
    })
}

pub(in crate::modules::vocabulary) fn filtered(
    mut query: sea_orm::Select<cards::Entity>,
    topic: Option<String>,
    text: Option<String>,
) -> sea_orm::Select<cards::Entity> {
    if let Some(topic) = topic.filter(|t| !t.trim().is_empty()) {
        query = query.filter(Expr::col((cards::Entity, cards::Column::Topics)).contains(
            Expr::value(serde_json::Value::Array(vec![topic.trim().into()])),
        ));
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

struct Statistics {
    total: u64,
    reviewable: u64,
    word_count: i64,
    topics: Vec<String>,
}
async fn statistics(
    db: &sea_orm::DatabaseConnection,
    base: sea_orm::Select<cards::Entity>,
    topic: Option<&str>,
    text: Option<&str>,
) -> Result<Statistics, AppError> {
    let source = base.build(sea_orm::DbBackend::Postgres);
    let mut values = source.values.map(|v| v.0).unwrap_or_default();
    let mut conditions = vec!["TRUE".to_owned()];
    if let Some(topic) = topic.map(str::trim).filter(|s| !s.is_empty()) {
        let index = values.len() + 1;
        values.push(serde_json::Value::Array(vec![topic.into()]).into());
        conditions.push(format!("c.topics @> ${index}::jsonb"));
    }
    if let Some(text) = text.map(str::trim).filter(|s| !s.is_empty()) {
        let index = values.len() + 1;
        values.push(crate::pagination::like_contains(text).into());
        conditions.push(format!(
            "(LOWER(c.word) LIKE ${index} ESCAPE '!' OR LOWER(c.meaning) LIKE ${index} ESCAPE '!')"
        ));
    }
    let condition = conditions.join(" AND ");
    let sql = format!(
        r#"WITH base AS ({source}) SELECT
        COUNT(*) FILTER (WHERE {condition}) AS total,
        COUNT(*) FILTER (WHERE ({condition}) AND c.meaning <> '') AS reviewable,
        COUNT(DISTINCT c.word_key) FILTER (WHERE {condition}) AS word_count,
        (SELECT COALESCE(jsonb_agg(DISTINCT topic ORDER BY topic), '[]'::jsonb) FROM base, jsonb_array_elements_text(topics) t(topic)) AS topics
        FROM base c"#,
        source = source.sql
    );
    let row = db
        .query_one(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| AppError::Internal("Missing vocabulary statistics".into()))?;
    Ok(Statistics {
        total: row.try_get::<i64>("", "total")? as u64,
        reviewable: row.try_get::<i64>("", "reviewable")? as u64,
        word_count: row.try_get("", "word_count")?,
        topics: serde_json::from_value(row.try_get("", "topics")?)
            .map_err(|e| AppError::Internal(e.to_string()))?,
    })
}
