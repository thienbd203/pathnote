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

#[tokio::test]
async fn workspace_filters_paginates_and_neighbors_stay_inside_owned_parent() {
    let app = LearningApp::spawn().await;
    let p = place(&app, &app.owner_token).await;
    let p2 = place(&app, &app.owner_token).await;
    let other = place(&app, &app.other_token).await;
    let a = session(
        &app,
        p,
        &app.owner_token,
        "A",
        "2020-01-01T10:00:00Z",
        "UTC",
    )
    .await;
    let b = session(
        &app,
        p,
        &app.owner_token,
        "B",
        "2020-01-01T10:00:00Z",
        "UTC",
    )
    .await;
    let future = session(
        &app,
        p,
        &app.owner_token,
        "Future",
        "2099-01-01T10:00:00Z",
        "UTC",
    )
    .await;
    session(
        &app,
        p2,
        &app.owner_token,
        "Elsewhere",
        "2020-01-01T10:00:00Z",
        "UTC",
    )
    .await;
    let hidden = session(
        &app,
        other,
        &app.other_token,
        "Secret",
        "2020-01-01T10:00:00Z",
        "UTC",
    )
    .await;
    let course = app
        .api
        .post(&format!("/api/v1/places/{p}/courses"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"IELTS","starts_on":"2020-01-01"}))
        .await;
    course.assert_status(StatusCode::CREATED);
    let course_id = course.json::<Value>()["data"]["id"].as_i64().unwrap();
    let lesson = app.api.post(&format!("/api/v1/courses/{course_id}/sessions"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"title":"Course lesson","starts_at":"2020-01-01T10:00:00Z","duration_minutes":30,"timezone":"UTC"})).await;
    lesson.assert_status(StatusCode::CREATED);
    for (page, expected) in [(1, b), (2, a)] {
        let r = app
            .api
            .get("/api/v1/sessions/search")
            .add_header("authorization", bearer(&app.owner_token))
            .add_query_param("timezone", "UTC")
            .add_query_param("place_id", p)
            .add_query_param("independent", true)
            .add_query_param("period", "past")
            .add_query_param("order", "desc")
            .add_query_param("per_page", 1)
            .add_query_param("page", page)
            .await;
        r.assert_status_ok();
        assert_eq!(r.json::<Value>()["data"][0]["id"], expected);
        assert_eq!(r.json::<Value>()["meta"]["total"], 2);
    }
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{b}/neighbors"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["previous"]["id"], a);
    assert_eq!(r.json::<Value>()["data"]["next"]["id"], future);
    app.api
        .get(&format!("/api/v1/sessions/{hidden}/neighbors"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    let r = app
        .api
        .get("/api/v1/courses/search")
        .add_header("authorization", bearer(&app.other_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 0);
    let r = app
        .api
        .get("/api/v1/courses/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("q", "ielts")
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"][0]["id"], course_id);
    app.api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "UTC")
        .add_query_param("place_id", other)
        .await
        .assert_status_not_found();
    app.api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "UTC")
        .add_query_param("course_id", course_id)
        .add_query_param("independent", true)
        .await
        .assert_status_unprocessable_entity();
    let course_lesson = lesson.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{course_lesson}/neighbors"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    r.assert_status_ok();
    assert!(r.json::<Value>()["data"]["previous"].is_null());
    assert!(r.json::<Value>()["data"]["next"].is_null());
    app.api
        .delete(&format!("/api/v1/sessions/{a}"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{b}/neighbors"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    r.assert_status_ok();
    assert!(r.json::<Value>()["data"]["previous"].is_null());
    let r = app
        .api
        .get("/api/v1/sessions/search")
        .add_header("authorization", bearer(&app.owner_token))
        .add_query_param("timezone", "UTC")
        .add_query_param("place_id", p)
        .add_query_param("period", "upcoming")
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 1);
    assert_eq!(r.json::<Value>()["data"][0]["id"], future);
    app.cleanup().await;
}
