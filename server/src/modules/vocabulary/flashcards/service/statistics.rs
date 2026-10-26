use super::{dto, runs};
use crate::{
    entities::{flashcard_attempts as attempts, flashcard_study_time as study_time},
    AppError,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use sea_orm::{
    sea_query::{Expr, OnConflict, Query},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, QueryTrait, Set, Statement,
};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Short immutable intervals make retries idempotent. PostgreSQL range_agg unions
/// overlapping tabs/devices before summing, so the same wall-clock time counts once.
pub async fn record_time(
    db: &DatabaseConnection,
    user: i64,
    id: Uuid,
    input: dto::StudyTimeInput,
) -> Result<dto::StudyTimeRecorded, AppError> {
    let run = runs::load(db, user, id).await?;
    let now = Utc::now().fixed_offset();
    let invalid = || AppError::validation("segments", "Khoảng thời gian ôn không hợp lệ.");
    if input.segments.is_empty() || input.segments.len() > 12 {
        return Err(invalid());
    }
    for segment in &input.segments {
        let duration = segment.ended_at - segment.started_at;
        if duration <= Duration::zero()
            || duration > Duration::seconds(15)
            || segment.started_at < run.started_at - Duration::seconds(5)
            || segment.started_at < now - Duration::hours(24)
            || segment.ended_at > now + Duration::seconds(5)
        {
            return Err(invalid());
        }
    }
    // A run must have actually served a card. No notes, answers or schedules are modified.
    let shown_query = Query::select()
        .expr_as(
            Expr::exists(
                attempts::Entity::find()
                    .select_only()
                    .column(attempts::Column::Id)
                    .filter(attempts::Column::UserId.eq(user))
                    .filter(attempts::Column::RunId.eq(id))
                    .into_query(),
            ),
            "shown",
        )
        .to_owned();
    let shown = db
        .query_one(db.get_database_backend().build(&shown_query))
        .await?
        .expect("EXISTS always returns a row");
    if !shown.try_get::<bool>("", "shown")? {
        return Err(invalid());
    }
    let mut models = Vec::new();
    for segment in input.segments {
        let start = segment.started_at.max(run.started_at);
        let end = segment
            .ended_at
            .min(now)
            .min(run.finished_at.unwrap_or(now));
        if end <= start {
            continue;
        }
        models.push(study_time::ActiveModel {
            id: Set(segment.id),
            user_id: Set(user),
            run_id: Set(id),
            started_at: Set(start),
            ended_at: Set(end),
        });
    }
    if !models.is_empty() {
        // One atomic INSERT; conflicts never overwrite another user's interval.
        study_time::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(study_time::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(db)
            .await?;
    }
    Ok(dto::StudyTimeRecorded { recorded: true })
}

fn bounds(day: NaiveDate, period: dto::StatisticsPeriod) -> (NaiveDate, NaiveDate) {
    use dto::StatisticsPeriod::*;
    let start = match period {
        Week => day - Duration::days(day.weekday().num_days_from_monday().into()),
        Month => day.with_day(1).unwrap(),
        Year => NaiveDate::from_ymd_opt(day.year(), 1, 1).unwrap(),
    };
    let end = match period {
        Week => start + Duration::days(7),
        Month if start.month() == 12 => NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap(),
        Month => NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap(),
        Year => NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap(),
    };
    (start, end)
}

pub async fn statistics(
    db: &DatabaseConnection,
    user: i64,
    input: dto::StatisticsInput,
) -> Result<dto::StudyStatistics, AppError> {
    let timezone = input.timezone.as_deref().unwrap_or("UTC");
    let tz: chrono_tz::Tz = timezone
        .parse()
        .map_err(|_| AppError::validation("timezone", "Múi giờ không hợp lệ."))?;
    let now = Utc::now();
    let today = now.with_timezone(&tz).date_naive();
    let day = input.date.unwrap_or(today);
    if !(2000..=2100).contains(&day.year()) {
        return Err(AppError::validation(
            "date",
            "Ngày phải trong khoảng năm 2000–2100.",
        ));
    }
    let (start, end) = bounds(day, input.period);
    // Calendar boundaries use the requested IANA zone, including 23/25-hour DST days.
    // Aggregate in PostgreSQL instead of returning every heartbeat to the application.
    let rows = db.query_all(Statement::from_sql_and_values(DbBackend::Postgres, r#"
        WITH days AS (
            SELECT generate_series($2::date::timestamp, ($3::date - 1)::timestamp, INTERVAL '1 day')::date AS day
            UNION SELECT $4::date
        ), windows AS (
            SELECT day, day::timestamp AT TIME ZONE $5 AS lo,
                (day + 1)::timestamp AT TIME ZONE $5 AS hi FROM days
        ), merged AS (
            SELECT w.day, range_agg(tstzrange(GREATEST(t.started_at,w.lo), LEAST(t.ended_at,w.hi), '[)')) AS spans
            FROM windows w JOIN flashcard_study_time t
                ON t.user_id=$1 AND t.started_at < w.hi AND t.started_at >= w.lo - INTERVAL '15 seconds' AND t.ended_at > w.lo
            GROUP BY w.day
        )
        SELECT day, SUM(EXTRACT(EPOCH FROM (upper(span)-lower(span))))::double precision AS seconds
        FROM merged CROSS JOIN LATERAL unnest(spans) AS span GROUP BY day ORDER BY day
    "#, [user.into(), start.into(), end.into(), today.into(), timezone.into()])).await?;
    let daily: BTreeMap<NaiveDate, f64> = rows
        .into_iter()
        .map(|row| Ok((row.try_get("", "day")?, row.try_get("", "seconds")?)))
        .collect::<Result<_, sea_orm::DbErr>>()?;
    let mut buckets: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut date = start;
    while date < end {
        let key = if matches!(input.period, dto::StatisticsPeriod::Year) {
            date.with_day(1).unwrap()
        } else {
            date
        };
        *buckets.entry(key).or_default() += daily.get(&date).copied().unwrap_or_default();
        date = date.succ_opt().unwrap();
    }
    let performance = performance(
        db,
        user,
        start,
        end,
        timezone,
        input.period,
        now.fixed_offset(),
    )
    .await?;
    Ok(dto::StudyStatistics {
        performance,
        period: input.period,
        start_date: start,
        end_date: end.pred_opt().unwrap(),
        today,
        today_seconds: daily.get(&today).copied().unwrap_or_default(),
        total_seconds: buckets.values().sum(),
        buckets: buckets
            .into_iter()
            .map(|(date, seconds)| dto::StudyTimeBucket { date, seconds })
            .collect(),
        as_of: now.fixed_offset(),
    })
}

/// Personal graded-attempt counts, independent of current source wording/access.
/// Only aggregate counts leave this function; no note, answer or other user's data.
async fn performance(
    db: &DatabaseConnection,
    user: i64,
    start: NaiveDate,
    end: NaiveDate,
    timezone: &str,
    period: dto::StatisticsPeriod,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> Result<dto::ReviewPerformance, AppError> {
    // Uses the existing (user_id, reviewed_at DESC) index. No attempt bodies/N+1 queries.
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        SELECT (reviewed_at AT TIME ZONE $4)::date AS day,
            COUNT(*) FILTER (WHERE rating=1) AS again,
            COUNT(*) FILTER (WHERE rating=2) AS hard,
            COUNT(*) FILTER (WHERE rating=3) AS good,
            COUNT(*) FILTER (WHERE rating=4) AS easy
        FROM flashcard_attempts
        WHERE user_id=$1 AND rating IN (1,2,3,4)
            AND reviewed_at >= ($2::date::timestamp AT TIME ZONE $4)
            AND reviewed_at < ($3::date::timestamp AT TIME ZONE $4)
            AND reviewed_at <= $5
        GROUP BY day ORDER BY day
    "#,
            [
                user.into(),
                start.into(),
                end.into(),
                timezone.into(),
                now.into(),
            ],
        ))
        .await?;
    let mut buckets: BTreeMap<NaiveDate, [u64; 4]> = BTreeMap::new();
    let key = |date: NaiveDate| {
        if matches!(period, dto::StatisticsPeriod::Year) {
            date.with_day(1).unwrap()
        } else {
            date
        }
    };
    let mut date = start;
    while date < end {
        buckets.entry(key(date)).or_default();
        date = date.succ_opt().unwrap();
    }
    let mut ratings = [0u64; 4];
    for row in rows {
        let day: NaiveDate = row.try_get("", "day")?;
        let counts = buckets
            .get_mut(&key(day))
            .expect("Query is bounded to the requested period");
        for (i, name) in ["again", "hard", "good", "easy"].iter().enumerate() {
            let count = row.try_get::<i64>("", name)? as u64;
            counts[i] += count;
            ratings[i] += count;
        }
    }
    let total: u64 = ratings.iter().sum();
    Ok(dto::ReviewPerformance {
        total,
        ratings,
        remembered_rate: (total > 0).then(|| (ratings[2] + ratings[3]) as f64 / total as f64),
        buckets: buckets
            .into_iter()
            .map(|(date, ratings)| dto::ReviewPerformanceBucket { date, ratings })
            .collect(),
    })
}
