use super::{learning_helpers::LearningApp, media::lesson};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use server::entities::exercises;
use std::future::IntoFuture;
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
fn choice() -> Value {
    json!({"type":"choice","prompt":"My sister ___ a teacher.","options":["am","is","are"],"answers":["is"],"explanation":"She uses is.","caseSensitive":false})
}
fn fill() -> Value {
    json!({"type":"fill","prompt":"She works ___ a teacher.","options":[],"answers":["as"],"explanation":"Work as + job.","caseSensitive":false})
}
async fn create(app: &LearningApp, session: i64, q: Value, token: &str) -> i64 {
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{session}/exercises"))
        .add_header("authorization", auth(token))
        .json(&json!({"title":" Jobs ","questions":[q]}))
        .await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
async fn start(app: &LearningApp, path: &str, token: &str, restart: bool) -> Value {
    let r = app
        .api
        .post(&format!("{path}/attempts"))
        .add_header("authorization", auth(token))
        .json(&json!({"restart":restart}))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn exercise_import_validates_all_questions_and_note_references_in_rest_and_bridge() {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let other = lesson(&app).await;
    let token = &app.owner_token;
    let base = format!("/api/v1/sessions/{s}/exercises");
    app.api
        .post(&base)
        .json(&json!({"title":"Jobs","questions":[choice()]}))
        .await
        .assert_status_unauthorized();
    app.api
        .post(&base)
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"title":"Jobs","questions":[choice()]}))
        .await
        .assert_status_not_found();
    let mut bad = choice();
    bad["answers"] = json!(["wrong"]);
    for questions in [
        json!([choice(), fill()]),
        json!([bad]),
        json!([{ "type":"translation","prompt":"hi" }]),
    ] {
        app.api
            .post(&base)
            .add_header("authorization", auth(token))
            .json(&json!({"title":"Jobs","questions":questions}))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .post(&format!("{base}/import"))
        .add_header("authorization", auth(token))
        .json(&json!({"title":"Jobs","questions":[choice(),{"type":"unsupported"}]}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        exercises::Entity::find()
            .filter(exercises::Column::SessionId.eq(s))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        0
    );
    let r=app.api.post(&format!("{base}/import")).add_header("authorization",auth(token)).json(&json!({"title":" Jobs ","questions":[choice(),fill()],"titles":[" Grammar "," Jobs "]})).await;
    r.assert_status(StatusCode::CREATED);
    let ids = r.json::<Value>()["data"]["ids"].as_array().unwrap().clone();
    assert_eq!(ids.len(), 2);
    let doc = json!({"type":"doc","content":ids.iter().map(|id|json!({"type":"exercise","attrs":{"exerciseId":id}})).collect::<Vec<_>>()});
    let payload = json!({"content":doc,"schema_version":4,"base_revision":0});
    app.api
        .put(&format!("/api/v1/sessions/{other}/note"))
        .add_header("authorization", auth(token))
        .json(&payload)
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.api
        .put(&format!("/api/v1/sessions/{s}/note"))
        .add_header("authorization", auth(token))
        .json(&payload)
        .await
        .assert_status_ok();
    app.cleanup().await;
}
#[tokio::test]
async fn choice_two_attempts_hide_answers_retry_once_and_revision_conflicts() {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let id = create(&app, s, choice(), &app.owner_token).await;
    let base = format!("/api/v1/sessions/{s}/exercises/{id}");
    let token = auth(&app.owner_token);
    let r = app.api.get(&base).add_header("authorization", &token).await;
    r.assert_status_ok();
    assert!(!r.text().contains("answers"));
    assert!(!r.text().contains("She uses"));
    let a = start(&app, &base, &app.owner_token, false).await;
    let aid = a["id"].as_i64().unwrap();
    let path = format!("{base}/attempts/{aid}/answers");
    app.api
        .post(&path)
        .add_header("authorization", &token)
        .json(&json!({"position":0,"answer":"outside","try":0}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body = json!({"position":0,"answer":"am","try":0});
    let (r1, r2) = tokio::join!(
        app.api
            .post(&path)
            .add_header("authorization", &token)
            .json(&body)
            .into_future(),
        app.api
            .post(&path)
            .add_header("authorization", &token)
            .json(&body)
            .into_future()
    );
    r1.assert_status_ok();
    r2.assert_status_ok();
    assert_eq!(r1.json::<Value>(), r2.json::<Value>());
    assert!(!r1.text().contains("answers"));
    assert_eq!(r1.json::<Value>()["data"]["results"][0]["tries"], 1);
    let r = app
        .api
        .post(&path)
        .add_header("authorization", &token)
        .json(&json!({"position":0,"answer":"are","try":1}))
        .await;
    r.assert_status_ok();
    let result = r.json::<Value>()["data"].clone();
    assert_eq!(result["completed"], true);
    assert_eq!(result["results"][0]["answers"], json!(["is"]));
    let r = app.api.get(&base).add_header("authorization", &token).await;
    assert_eq!(r.json::<Value>()["data"]["attempt"], result);
    app.api
        .post(&path)
        .add_header("authorization", &token)
        .json(&json!({"position":0,"answer":"is","try":1}))
        .await
        .assert_status(StatusCode::CONFLICT);
    let new = start(&app, &base, &app.owner_token, true).await;
    assert_ne!(new["id"], aid);
    assert!(new["results"].as_array().unwrap().is_empty());
    let definition = json!({"title":"New","questions":[fill()],"revision":1});
    app.api
        .put(&base)
        .add_header("authorization", &token)
        .json(&definition)
        .await
        .assert_status_ok();
    app.api
        .put(&base)
        .add_header("authorization", &token)
        .json(&definition)
        .await
        .assert_status(StatusCode::CONFLICT);
    app.api
        .post(&format!("{base}/attempts/{}/answers", new["id"]))
        .add_header("authorization", &token)
        .json(&json!({"position":0,"answer":"is","try":0}))
        .await
        .assert_status(StatusCode::CONFLICT);
    app.cleanup().await;
}
#[tokio::test]
async fn fill_normalizes_whitespace_case_results_are_private_and_revocation_blocks_access() {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let owner = auth(&app.owner_token);
    let guest = auth(&app.other_token);
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{s}/collaborators"))
        .add_header("authorization", &owner)
        .json(&json!({"email":"other@example.test"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let id = create(&app, s, fill(), &app.owner_token).await;
    let base = format!("/api/v1/sessions/{s}/exercises/{id}");
    app.api
        .get(&format!("{base}/definition"))
        .add_header("authorization", &guest)
        .await
        .assert_status_not_found();
    let a = start(&app, &base, &app.other_token, false).await;
    let path = format!("{base}/attempts/{}/answers", a["id"]);
    let r = app
        .api
        .post(&path)
        .add_header("authorization", &guest)
        .json(&json!({"position":0,"answer":" AS ","try":0}))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["results"][0]["correct"], true);
    let r = app.api.get(&base).add_header("authorization", &owner).await;
    assert!(r.json::<Value>()["data"]["attempt"].is_null());
    assert!(!r.text().contains("answers"));
    app.api
        .post(&path)
        .add_header("authorization", &owner)
        .json(&json!({"position":0,"answer":"as","try":0}))
        .await
        .assert_status_not_found();
    let collaborator_created = create(&app, s, fill(), &app.other_token).await;
    app.api
        .get(&format!(
            "/api/v1/sessions/{s}/exercises/{collaborator_created}/definition"
        ))
        .add_header("authorization", &guest)
        .await
        .assert_status_ok();
    let members = app
        .api
        .get(&format!("/api/v1/sessions/{s}/collaborators"))
        .add_header("authorization", &owner)
        .await
        .json::<Value>();
    let uid = members["data"][0]["id"].as_i64().unwrap();
    app.api
        .delete(&format!("/api/v1/sessions/{s}/collaborators/{uid}"))
        .add_header("authorization", &owner)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    for path in [base.clone(), format!("{base}/definition")] {
        app.api
            .get(&path)
            .add_header("authorization", &guest)
            .await
            .assert_status_not_found();
    }
    app.api
        .post(&path)
        .add_header("authorization", &guest)
        .json(&json!({"position":0,"answer":"as","try":0}))
        .await
        .assert_status_not_found();
    app.api
        .delete(&format!("/api/v1/sessions/{s}"))
        .add_header("authorization", &owner)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .get(&base)
        .add_header("authorization", &owner)
        .await
        .assert_status_not_found();
    app.cleanup().await;
}

#[tokio::test]
async fn exercise_references_roundtrip_through_realtime_snapshot_and_reject_other_lesson() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let s = lesson(&app).await;
    let other = lesson(&app).await;
    let id = create(&app, s, choice(), &app.owner_token).await;
    let doc = json!({"type":"doc","content":[{"type":"exercise","attrs":{"exerciseId":id}}]});
    let internal = format!("/api/v1/internal/notes/{s}");
    let r = app
        .api
        .put(&internal)
        .add_header(
            "x-collaboration-secret",
            "test-collaboration-secret-32-characters",
        )
        .json(&json!({"content":doc,"schema_version":4,"state":"AAA=","base_revision":0}))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["content"], doc);
    app.api
        .put(&format!("/api/v1/internal/notes/{other}"))
        .add_header(
            "x-collaboration-secret",
            "test-collaboration-secret-32-characters",
        )
        .json(&json!({"content":doc,"schema_version":4,"state":"AAA=","base_revision":0}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.cleanup().await;
}

async fn bulk(app: &LearningApp, session: i64, ids: &str, token: &str) -> Value {
    let response = app
        .api
        .get(&format!("/api/v1/sessions/{session}/exercises"))
        .add_query_param("ids", ids)
        .add_header("authorization", auth(token))
        .await;
    response.assert_status_ok();
    response.json::<Value>()["data"].clone()
}

#[tokio::test]
async fn bulk_exercises_are_bounded_session_scoped_and_use_the_public_projection() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    let another = lesson(&app).await;
    let id = create(&app, session, choice(), &app.owner_token).await;
    let foreign = create(&app, another, fill(), &app.owner_token).await;
    let url = format!("/api/v1/sessions/{session}/exercises");
    app.api
        .get(&url)
        .add_query_param("ids", id)
        .await
        .assert_status_unauthorized();
    app.api
        .get(&url)
        .add_query_param("ids", id)
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .get("/api/v1/sessions/999999/exercises")
        .add_query_param("ids", id)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status_not_found();
    for invalid in [
        "".to_owned(),
        "0".into(),
        "-1".into(),
        "1,no".into(),
        "1,".into(),
        " 1".into(),
        "9223372036854775808".into(),
        vec!["1"; 101].join(","),
    ] {
        app.api
            .get(&url)
            .add_query_param("ids", invalid)
            .add_header("authorization", auth(&app.owner_token))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .get(&url)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.api
        .get(&url)
        .add_query_param("ids", id)
        .add_query_param("definition", true)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let rows = bulk(
        &app,
        session,
        &format!("{foreign},{id},999999,{id}"),
        &app.owner_token,
    )
    .await;
    let single = app
        .api
        .get(&format!("{url}/{id}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    assert_eq!(rows, json!([single["data"]]));
    assert!(!rows.to_string().contains("answers"));
    assert!(!rows.to_string().contains("explanation"));
    assert_eq!(
        bulk(&app, session, &foreign.to_string(), &app.owner_token).await,
        json!([])
    );
    let imported = app
        .api
        .post(&format!("{url}/import"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"title":"100 questions","questions":vec![choice();100]}))
        .await;
    imported.assert_status(StatusCode::CREATED);
    let ids = imported.json::<Value>()["data"]["ids"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .map(|v| v.as_i64().unwrap().to_string())
        .collect::<Vec<_>>();
    let rows = bulk(&app, session, &ids.join(","), &app.owner_token).await;
    assert_eq!(rows.as_array().unwrap().len(), 100);
    assert_eq!(rows[0]["id"].as_i64().unwrap().to_string(), ids[0]);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_attempts_are_personal_latest_current_revision_and_revocation_safe() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    let owner = auth(&app.owner_token);
    let guest = auth(&app.other_token);
    let member = app
        .api
        .post(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", &owner)
        .json(&json!({"email":"other@example.test"}))
        .await;
    member.assert_status(StatusCode::CREATED);
    let id = create(&app, session, fill(), &app.owner_token).await;
    let other_id = create(&app, session, choice(), &app.other_token).await;
    let path = format!("/api/v1/sessions/{session}/exercises/{id}");
    let a = start(&app, &path, &app.other_token, false).await;
    app.api
        .post(&format!("{path}/attempts/{}/answers", a["id"]))
        .add_header("authorization", &guest)
        .json(&json!({"position":0,"answer":"wrong","try":0}))
        .await
        .assert_status_ok();
    let rows = bulk(&app, session, &format!("{id},{other_id}"), &app.other_token).await;
    assert_eq!(rows[0]["can_manage"], false);
    assert_eq!(rows[1]["can_manage"], true);
    assert!(!rows.to_string().contains("answers"));
    app.api
        .post(&format!("{path}/attempts/{}/answers", a["id"]))
        .add_header("authorization", &guest)
        .json(&json!({"position":0,"answer":"wrong","try":1}))
        .await
        .assert_status_ok();
    let rows = bulk(&app, session, &id.to_string(), &app.other_token).await;
    assert_eq!(rows[0]["attempt"]["results"][0]["answers"], json!(["as"]));
    let single = app
        .api
        .get(&path)
        .add_header("authorization", &guest)
        .await
        .json::<Value>();
    assert_eq!(rows[0], single["data"]);
    let owner_rows = bulk(&app, session, &id.to_string(), &app.owner_token).await;
    assert!(owner_rows[0]["attempt"].is_null());
    assert!(!owner_rows.to_string().contains("answers"));
    let restart = start(&app, &path, &app.other_token, true).await;
    let rows = bulk(&app, session, &id.to_string(), &app.other_token).await;
    assert_eq!(rows[0]["attempt"]["id"], restart["id"]);
    assert!(!rows.to_string().contains("answers"));
    app.api
        .put(&path)
        .add_header("authorization", &owner)
        .json(&json!({"title":"New revision","questions":[fill()],"revision":1}))
        .await
        .assert_status_ok();
    let rows = bulk(&app, session, &id.to_string(), &app.other_token).await;
    assert_eq!(rows[0]["revision"], 2);
    assert!(rows[0]["attempt"].is_null());
    let current = start(&app, &path, &app.other_token, false).await;
    assert_eq!(
        bulk(&app, session, &id.to_string(), &app.other_token).await[0]["attempt"]["id"],
        current["id"]
    );
    let members = app
        .api
        .get(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", &owner)
        .await
        .json::<Value>();
    let uid = members["data"][0]["id"].as_i64().unwrap();
    app.api
        .delete(&format!("/api/v1/sessions/{session}/collaborators/{uid}"))
        .add_header("authorization", &owner)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .get(&format!("/api/v1/sessions/{session}/exercises"))
        .add_query_param("ids", id)
        .add_header("authorization", &guest)
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
