use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use serde_json::{json, Value};
fn bearer(s: &str) -> String {
    format!("Bearer {s}")
}
async fn place(app: &LearningApp, token: &str) -> i64 {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", bearer(token))
        .json(&json!({"name":"Search school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
async fn session(
    app: &LearningApp,
    p: i64,
    token: &str,
    title: &str,
    start: &str,
    tz: &str,
) -> i64 {
    let r = app
        .api
        .post(&format!("/api/v1/places/{p}/sessions"))
        .add_header("authorization", bearer(token))
        .json(&json!({"title":title,"starts_at":start,"duration_minutes":30,"timezone":tz}))
        .await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
#[tokio::test]
async fn search_uses_local_day_boundaries_and_owner_scope_outside_the_or_match() {
    let app = LearningApp::spawn().await;
    let p = place(&app, &app.owner_token).await;
    let other = place(&app, &app.other_token).await;
    let first = session(
        &app,
        p,
        &app.owner_token,
        "Jobs",
        "2026-10-12T17:00:00Z",
        "Asia/Ho_Chi_Minh",
    )
    .await;
    session(
        &app,
        p,
        &app.owner_token,
        "Late",
        "2026-10-13T16:00:00Z",
        "Asia/Ho_Chi_Minh",
    )
    .await;
    session(
        &app,
        p,
        &app.owner_token,
        "Next day",
        "2026-10-13T17:00:00Z",
        "Asia/Ho_Chi_Minh",
    )
    .await;
    session(
        &app,
        other,
        &app.other_token,
        "Other Jobs",
        "2026-10-13T11:00:00Z",
        "Asia/Ho_Chi_Minh",
    )
    .await;
    let r = app
        .api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("q", "13/10/2026")
        .add_query_param("timezone", "Asia/Saigon")
        .add_query_param("per_page", 1)
        .await;
    r.assert_status_ok();
    let d = r.json::<Value>();
    assert_eq!(d["meta"]["total"], 2);
    assert_eq!(d["data"][0]["id"], first);
    assert_eq!(d["meta"]["last_page"], 2);
    let r = app
        .api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("q", "JOBS")
        .add_query_param("timezone", "Asia/Saigon")
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 1);
    app.cleanup().await;
}
#[tokio::test]
async fn search_dst_day_has_twenty_three_hours_and_wildcards_are_literal() {
    let app = LearningApp::spawn().await;
    let p = place(&app, &app.owner_token).await;
    session(
        &app,
        p,
        &app.owner_token,
        "100%_ lesson",
        "2027-03-14T05:00:00Z",
        "America/New_York",
    )
    .await;
    session(
        &app,
        p,
        &app.owner_token,
        "Evening",
        "2027-03-15T03:00:00Z",
        "America/New_York",
    )
    .await;
    session(
        &app,
        p,
        &app.owner_token,
        "Following",
        "2027-03-15T04:00:00Z",
        "America/New_York",
    )
    .await;
    for (q, total) in [("14/3/2027", 2), ("%_", 1), ("31/2/2027", 0)] {
        let r = app
            .api
            .get("/api/v1/sessions/search")
            .add_header("authorization", bearer(&app.owner_token))
            .add_query_param("q", q)
            .add_query_param("timezone", "America/New_York")
            .await;
        r.assert_status_ok();
        assert_eq!(r.json::<Value>()["meta"]["total"], total);
    }
    app.api
        .get("/api/v1/sessions/search")
        .await
        .assert_status_unauthorized();
    app.api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "Mars/Fake")
        .await
        .assert_status_unprocessable_entity();
    app.api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "UTC")
        .add_query_param("course_id", 999999)
        .await
        .assert_status_not_found();
    app.api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "Pacific/Apia")
        .add_query_param("q", "30/12/2011")
        .await
        .assert_status_unprocessable_entity();
    app.cleanup().await;
}
