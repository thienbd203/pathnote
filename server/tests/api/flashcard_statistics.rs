use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{json, Value};
use uuid::Uuid;
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn run(app: &LearningApp) -> (String, String) {
    let session = lesson(app, &app.owner_token).await;
    save(app, session, &doc(vec![entry("time", "thời gian", &[])]), 0).await;
    let r = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let id = r.json::<Value>()["data"]["id"].as_str().unwrap().to_owned();
    let r = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"run_id":id}))
        .await;
    r.assert_status_ok();
    (
        id,
        r.json::<Value>()["data"]["card"]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}
async fn stats(app: &LearningApp, token: &str, query: &str) -> Value {
    let r = app
        .api
        .get(&format!("/api/v1/flashcards/statistics?{query}"))
        .add_header("authorization", auth(token))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn seed(app: &LearningApp, run: &str, start: &str, end: &str) {
    app.db.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO flashcard_study_time(id,user_id,run_id,started_at,ended_at) VALUES($1,$2,$3,$4::text::timestamptz,$5::text::timestamptz)",
        [Uuid::new_v4().into(),app.owner_id.into(),Uuid::parse_str(run).unwrap().into(),start.into(),end.into()])).await.unwrap();
}
#[tokio::test]
async fn records_are_idempotent_private_and_do_not_modify_cards_or_memory() {
    let app = LearningApp::spawn().await;
    let (run, ticket) = run(&app).await;
    // No wall-clock inference for old runs, even if a card has been answered.
    let initial = stats(&app, &app.owner_token, "period=week&timezone=UTC").await;
    assert_eq!(initial["total_seconds"], 0.0);
    app.db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE flashcard_runs SET started_at=started_at-INTERVAL '1 hour' WHERE id=$1",
            [Uuid::parse_str(&run).unwrap().into()],
        ))
        .await
        .unwrap();
    let end = Utc::now() - Duration::seconds(1);
    let first = json!({"id":Uuid::new_v4().to_string(),"started_at":end-Duration::seconds(10),"ended_at":end});
    let body = json!({"segments":[first]});
    for _ in 0..2 {
        app.api
            .post(&format!("/api/v1/flashcards/runs/{run}/time"))
            .add_header("authorization", auth(&app.owner_token))
            .json(&body)
            .await
            .assert_status_ok();
    }
    app.api
        .post(&format!("/api/v1/flashcards/runs/{run}/time"))
        .add_header("authorization", auth(&app.other_token))
        .json(&body)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.api
        .post(&format!("/api/v1/flashcards/runs/{run}/time"))
        .json(&body)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    let second = json!({"segments":[{"id":Uuid::new_v4().to_string(),"started_at":end-Duration::seconds(5),"ended_at":end+Duration::milliseconds(500)}]});
    app.api
        .post(&format!("/api/v1/flashcards/runs/{run}/time"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&second)
        .await
        .assert_status_ok();
    let data = stats(&app, &app.owner_token, "period=week&timezone=UTC").await;
    assert_eq!(data["today_seconds"], 10.5);
    assert_eq!(data["total_seconds"], 10.5);
    assert_eq!(
        stats(&app, &app.other_token, "period=week&timezone=UTC").await["today_seconds"],
        0.0
    );
    // Time recording neither reveals nor grades the ticket.
    let row = app
        .db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT revealed,rating FROM flashcard_attempts WHERE id=$1",
            [Uuid::parse_str(&ticket).unwrap().into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(!row.try_get::<bool>("", "revealed").unwrap());
    assert_eq!(row.try_get::<Option<i32>>("", "rating").unwrap(), None);
    app.cleanup().await;
}
#[tokio::test]
async fn intervals_split_at_local_midnight_and_week_month_year_totals_agree() {
    let app = LearningApp::spawn().await;
    let (run, _) = run(&app).await;
    seed(&app, &run, "2026-10-24T16:59:55Z", "2026-10-24T17:00:05Z").await;
    seed(&app, &run, "2026-10-24T16:59:58Z", "2026-10-24T17:00:08Z").await;
    let week = stats(
        &app,
        &app.owner_token,
        "period=week&date=2026-10-24&timezone=Asia%2FHo_Chi_Minh",
    )
    .await;
    assert_eq!(week["start_date"], "2026-10-19");
    assert_eq!(week["end_date"], "2026-10-25");
    assert_eq!(week["buckets"].as_array().unwrap().len(), 7);
    assert_eq!(week["buckets"][5]["seconds"], 5.0);
    assert_eq!(week["buckets"][6]["seconds"], 8.0);
    assert_eq!(week["total_seconds"], 13.0);
    let month = stats(
        &app,
        &app.owner_token,
        "period=month&date=2026-10-01&timezone=Asia%2FHo_Chi_Minh",
    )
    .await;
    assert_eq!(month["buckets"].as_array().unwrap().len(), 31);
    assert_eq!(month["total_seconds"], 13.0);
    let year = stats(
        &app,
        &app.owner_token,
        "period=year&date=2026-02-01&timezone=Asia%2FHo_Chi_Minh",
    )
    .await;
    assert_eq!(year["buckets"].as_array().unwrap().len(), 12);
    assert_eq!(year["buckets"][9]["seconds"], 13.0);
    assert_eq!(year["total_seconds"], 13.0);
    let empty = stats(
        &app,
        &app.owner_token,
        "period=month&date=2024-02-15&timezone=UTC",
    )
    .await;
    assert_eq!(empty["end_date"], "2024-02-29");
    assert_eq!(empty["buckets"].as_array().unwrap().len(), 29);
    assert_eq!(empty["total_seconds"], 0.0);
    app.cleanup().await;
}
#[tokio::test]
async fn timezone_dst_and_cross_year_week_use_calendar_boundaries() {
    let app = LearningApp::spawn().await;
    let (run, _) = run(&app).await;
    seed(&app, &run, "2026-11-01T03:59:55Z", "2026-11-01T04:00:05Z").await;
    seed(&app, &run, "2026-11-02T04:59:55Z", "2026-11-02T05:00:05Z").await;
    let data = stats(
        &app,
        &app.owner_token,
        "period=month&date=2026-11-01&timezone=America%2FNew_York",
    )
    .await;
    assert_eq!(data["buckets"][0]["seconds"], 10.0);
    assert_eq!(data["buckets"][1]["seconds"], 5.0);
    assert_eq!(data["total_seconds"], 15.0);
    let week = stats(
        &app,
        &app.owner_token,
        "period=week&date=2027-01-01&timezone=UTC",
    )
    .await;
    assert_eq!(week["start_date"], "2026-12-28");
    assert_eq!(week["end_date"], "2027-01-03");
    app.cleanup().await;
}
#[tokio::test]
async fn validates_batches_dates_and_clips_late_reports_to_run_finish() {
    let app = LearningApp::spawn().await;
    let (run, _) = run(&app).await;
    for query in [
        "period=bad",
        "timezone=bogus",
        "date=2101-01-01",
        "date=2026-02-30",
        "unknown=1",
    ] {
        app.api
            .get(&format!("/api/v1/flashcards/statistics?{query}"))
            .add_header("authorization", auth(&app.owner_token))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let now = Utc::now();
    let good = json!({"id":Uuid::new_v4().to_string(),"started_at":now-Duration::seconds(1),"ended_at":now});
    let bad = json!({"id":Uuid::new_v4().to_string(),"started_at":now-Duration::seconds(31),"ended_at":now});
    for body in [
        json!({"segments":[]}),
        json!({"segments":[good,bad]}),
        json!({"segments":vec![good.clone();13]}),
        json!({"segments":[{"id":Uuid::new_v4().to_string(),"started_at":now,"ended_at":now+Duration::minutes(2)}]}),
    ] {
        app.api
            .post(&format!("/api/v1/flashcards/runs/{run}/time"))
            .add_header("authorization", auth(&app.owner_token))
            .json(&body)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        stats(&app, &app.owner_token, "period=week").await["total_seconds"],
        0.0
    );
    let end = Utc::now() - Duration::seconds(2);
    app.db.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "UPDATE flashcard_runs SET started_at=$2::timestamptz-INTERVAL '1 hour',finished_at=$2 WHERE id=$1",[Uuid::parse_str(&run).unwrap().into(),end.fixed_offset().into()])).await.unwrap();
    let body = json!({"segments":[{"id":Uuid::new_v4().to_string(),"started_at":end-Duration::seconds(5),"ended_at":end+Duration::seconds(1)}]});
    app.api
        .post(&format!("/api/v1/flashcards/runs/{run}/time"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&body)
        .await
        .assert_status_ok();
    assert_eq!(
        stats(&app, &app.owner_token, "period=week").await["total_seconds"],
        5.0
    );
    app.cleanup().await;
}
