use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::flashcard_memory as memory;
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn next(app: &LearningApp, token: &str, filters: Value) -> Value {
    let r = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(token))
        .json(&filters)
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn reveal(app: &LearningApp, token: &str, id: &str) -> Value {
    let r = app
        .api
        .post(&format!("/api/v1/flashcards/{id}/reveal"))
        .add_header("authorization", auth(token))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn grade(app: &LearningApp, id: &str, rating: u8) -> Value {
    let r = app
        .api
        .post(&format!("/api/v1/flashcards/{id}/grade"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"rating":rating}))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn settings(app: &LearningApp, filters: Value, direction: &str, retention: f32) -> Value {
    let mut input = filters;
    input["direction"] = json!(direction);
    input["retention"] = json!(retention);
    let r = app
        .api
        .put("/api/v1/flashcards")
        .add_header("authorization", auth(&app.owner_token))
        .json(&input)
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn seed(app: &LearningApp) -> (i64, Value) {
    let id = lesson(app, &app.owner_token).await;
    let content = doc(vec![entry("teacher", "giáo viên", &["Jobs"])]);
    save(app, id, &content, 0).await;
    (id, content)
}
#[tokio::test]
async fn directions_settings_and_fsrs_intervals_are_independent_and_ratings_idempotent() {
    let app = LearningApp::spawn().await;
    let (id, _) = seed(&app).await;
    let f = json!({"session_id":id,"topic":"Jobs"});
    let first = next(&app, &app.owner_token, f.clone()).await;
    assert_eq!(first["overview"]["settings"]["direction"], "vi_en");
    assert_eq!(first["card"]["front"], "giáo viên");
    assert!(first["card"].get("word").is_none());
    assert!(first["card"]["audio"].is_null());
    let ticket = first["card"]["id"].as_str().unwrap();
    app.api
        .post(&format!("/api/v1/flashcards/{ticket}/grade"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"rating":4}))
        .await
        .assert_status(StatusCode::CONFLICT);
    let back = reveal(&app, &app.owner_token, ticket).await;
    assert_eq!(back["back"]["word"], "teacher");
    assert_eq!(back["options"][0]["seconds"], 60);
    assert_eq!(back["options"][2]["seconds"], 600);
    // Compare the public API to the actual package, rather than hardcoded day intervals.
    let expected = fsrs::FSRS::default()
        .next_states(None, 0.9, 0)
        .unwrap()
        .easy;
    assert_eq!(
        back["options"][3]["seconds"],
        expected.interval.round().max(1.0) as i64 * 86400
    );
    let (a, b) = tokio::join!(grade(&app, ticket, 4), grade(&app, ticket, 4));
    assert_eq!(a, b);
    let rows = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reviews, 1);
    assert_eq!(rows[0].stability, expected.memory.stability);
    assert_eq!(rows[0].difficulty, expected.memory.difficulty);
    assert!(next(&app, &app.owner_token, f.clone()).await["card"].is_null());
    assert_eq!(settings(&app, f.clone(), "both", 0.95).await["new"], 1);
    let reverse = next(&app, &app.owner_token, f.clone()).await;
    assert_eq!(reverse["card"]["direction"], "en_vi");
    assert_eq!(reverse["card"]["front"], "teacher");
    assert_eq!(reverse["card"]["audio"]["text"], "teacher");
    let reverse_id = reverse["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, reverse_id).await;
    grade(&app, reverse_id, 3).await;
    let config = app
        .api
        .get(&format!(
            "/api/v1/flashcards?session_id={id}&topic=Jobs&q=teacher"
        ))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    config.assert_status_ok();
    assert_eq!(
        config.json::<Value>()["data"]["settings"]["direction"],
        "both"
    );
    let outside = next(&app, &app.owner_token, json!({})).await;
    assert_eq!(outside["overview"]["settings"]["direction"], "vi_en");
    assert!(outside["card"].is_null());
    let rows = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].due_at, rows[1].due_at);
    app.cleanup().await;
}
#[tokio::test]
async fn learning_steps_due_priority_and_stale_tabs_use_current_memory() {
    let app = LearningApp::spawn().await;
    let (id, mut content) = seed(&app).await;
    let first = next(&app, &app.owner_token, json!({})).await;
    let stale = next(&app, &app.owner_token, json!({})).await;
    let ticket = first["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    let result = grade(&app, ticket, 1).await;
    let due = chrono::DateTime::parse_from_rfc3339(result["due_at"].as_str().unwrap()).unwrap();
    assert!((due - Utc::now().fixed_offset()).num_seconds() <= 60);
    let stale = stale["card"]["id"].as_str().unwrap();
    app.api
        .post(&format!("/api/v1/flashcards/{stale}/reveal"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::CONFLICT);
    let waiting = next(&app, &app.owner_token, json!({})).await;
    assert!(waiting["card"].is_null());
    assert_eq!(waiting["overview"]["learning"], 1);
    assert!(!waiting["overview"]["next_due_at"].is_null());
    let row = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut row: memory::ActiveModel = row.into();
    row.due_at = Set(Utc::now().fixed_offset() - Duration::minutes(1));
    row.update(app.db.as_ref()).await.unwrap();
    content["content"][0]["content"][0]["content"]
        .as_array_mut()
        .unwrap()
        .push(entry("chef", "đầu bếp", &["Jobs"]));
    save(&app, id, &content, 1).await;
    let due = next(&app, &app.owner_token, json!({})).await;
    assert_eq!(due["card"]["front"], "giáo viên");
    assert_eq!(due["card"]["new"], false);
    let t = due["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, t).await;
    grade(&app, t, 3).await;
    let row = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut row: memory::ActiveModel = row.into();
    row.due_at = Set(Utc::now().fixed_offset() - Duration::minutes(1));
    row.update(app.db.as_ref()).await.unwrap();
    let due = next(&app, &app.owner_token, json!({"q":"teacher"})).await;
    let t = due["card"]["id"].as_str().unwrap();
    let back = reveal(&app, &app.owner_token, t).await;
    assert!(back["options"][2]["seconds"].as_i64().unwrap() >= 86400);
    grade(&app, t, 3).await;
    assert_eq!(
        memory::Entity::find()
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .step,
        0
    );
    app.cleanup().await;
}
#[tokio::test]
async fn edits_and_revoked_access_cannot_reveal_or_grade_old_cards() {
    let app = LearningApp::spawn().await;
    let (id, mut content) = seed(&app).await;
    let first = next(&app, &app.owner_token, json!({})).await;
    let t = first["card"]["id"].as_str().unwrap();
    content["content"][0]["content"][0]["content"][0]["attrs"]["ipa"] = json!("/new/");
    save(&app, id, &content, 1).await;
    assert_eq!(
        reveal(&app, &app.owner_token, t).await["back"]["ipa"],
        "/new/"
    );
    grade(&app, t, 4).await;
    assert!(next(&app, &app.owner_token, json!({})).await["card"].is_null());
    content["content"][0]["content"][0]["content"][0]["content"][0]["attrs"]["meaning"] =
        json!("người giảng dạy");
    save(&app, id, &content, 2).await;
    app.api
        .post(&format!("/api/v1/flashcards/{t}/reveal"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::CONFLICT);
    let new = next(&app, &app.owner_token, json!({})).await;
    assert_eq!(new["card"]["new"], true);
    assert_eq!(new["card"]["front"], "người giảng dạy");
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{id}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let member = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let other = next(&app, &app.other_token, json!({})).await;
    assert_eq!(other["card"]["new"], true);
    let t = other["card"]["id"].as_str().unwrap();
    reveal(&app, &app.other_token, t).await;
    app.api
        .post(&format!("/api/v1/flashcards/{t}/reveal"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status_not_found();
    app.api
        .delete(&format!("/api/v1/sessions/{id}/collaborators/{member}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    for suffix in ["reveal", "grade"] {
        app.api
            .post(&format!("/api/v1/flashcards/{t}/{suffix}"))
            .add_header("authorization", auth(&app.other_token))
            .json(&json!({"rating":3}))
            .await
            .assert_status_not_found();
    }
    app.api
        .get(&format!("/api/v1/flashcards?session_id={id}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
#[tokio::test]
async fn invalid_settings_and_filters_are_rejected_and_empty_decks_work() {
    let app = LearningApp::spawn().await;
    let empty = next(&app, &app.owner_token, json!({})).await;
    assert!(empty["card"].is_null());
    assert_eq!(empty["overview"]["due"], 0);
    assert_eq!(empty["overview"]["new"], 0);
    for body in [
        json!({"direction":"bad","retention":0.9}),
        json!({"direction":"vi_en","retention":0.99}),
        json!({"direction":"vi_en","retention":0.9,"session_id":-1}),
        json!({"direction":"vi_en","retention":0.9,"extra":true}),
    ] {
        app.api
            .put("/api/v1/flashcards")
            .add_header("authorization", auth(&app.owner_token))
            .json(&body)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .post("/api/v1/flashcards/next")
        .json(&json!({}))
        .await
        .assert_status_unauthorized();
    for input in [
        json!({"q":"a".repeat(201)}),
        json!({"topic":"a".repeat(61)}),
        json!({"extra":true}),
    ] {
        app.api
            .post("/api/v1/flashcards/next")
            .add_header("authorization", auth(&app.owner_token))
            .json(&input)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        memory::Entity::find()
            .filter(memory::Column::UserId.eq(app.owner_id))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        0
    );
    app.cleanup().await;
}
