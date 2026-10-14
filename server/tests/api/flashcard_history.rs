use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use server::entities::{flashcard_attempts as attempts, flashcard_memory as memory};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn start(app: &LearningApp, token: &str, q: &str) -> String {
    let r = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(token))
        .json(&json!({"q":q}))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"]["card"]["id"]
        .as_str()
        .unwrap()
        .into()
}
async fn finish(app: &LearningApp, token: &str, id: &str, rating: u8) {
    app.api
        .post(&format!("/api/v1/flashcards/{id}/reveal"))
        .add_header("authorization", auth(token))
        .await
        .assert_status_ok();
    app.api
        .post(&format!("/api/v1/flashcards/{id}/grade"))
        .add_header("authorization", auth(token))
        .json(&json!({"rating":rating}))
        .await
        .assert_status_ok();
}
async fn history(app: &LearningApp, token: &str, query: &str) -> Value {
    let r = app
        .api
        .get(&format!("/api/v1/flashcards/history{query}"))
        .add_header("authorization", auth(token))
        .await;
    r.assert_status_ok();
    r.json()
}
#[tokio::test]
async fn history_preserves_graded_snapshot_after_edits_removal_and_never_changes_memory() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    let mut content = doc(vec![entry("teacher", "giáo viên", &["Jobs"])]);
    save(&app, session, &content, 0).await;
    let first = start(&app, &app.owner_token, "teacher").await;
    assert_eq!(
        history(&app, &app.owner_token, "").await["meta"]["total"],
        0
    );
    finish(&app, &app.owner_token, &first, 1).await;
    app.api
        .post(&format!("/api/v1/flashcards/{first}/grade"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"rating":1}))
        .await
        .assert_status_ok();
    content["content"][0]["content"][0]["content"][0]["content"][0]["attrs"]["meaning"] =
        json!("người giảng dạy");
    content["content"][0]["content"][0]["content"][0]["attrs"]["ipa"] = json!("/new/");
    content["content"][0]["content"][0]["content"][0]["attrs"]["topics"] = json!(["School"]);
    save(&app, session, &content, 1).await;
    let second = start(&app, &app.owner_token, "teacher").await;
    finish(&app, &app.owner_token, &second, 4).await;
    let before = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    let list = history(&app, &app.owner_token, "").await;
    assert_eq!(list["meta"]["total"], 2);
    assert_eq!(list["data"][0]["rating"], 4);
    assert_eq!(list["data"][0]["card"]["meaning"], "người giảng dạy");
    let old = history(&app, &app.owner_token, "?q=gi%C3%A1o&topic=Jobs&rating=1").await;
    assert_eq!(old["meta"]["total"], 1);
    assert_eq!(old["data"][0]["id"], first);
    assert_eq!(old["data"][0]["snapshot"], true);
    assert_eq!(old["data"][0]["card"]["meaning"], "giáo viên");
    assert_ne!(old["data"][0]["card"]["ipa"], "/new/");
    assert_eq!(
        old["data"][0]["card"]["examples"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        memory::Entity::find().all(app.db.as_ref()).await.unwrap(),
        before
    );
    save(
        &app,
        session,
        &json!({"type":"doc","content":[{"type":"paragraph"}]}),
        2,
    )
    .await;
    assert_eq!(
        history(&app, &app.owner_token, "").await["meta"]["total"],
        2
    );
    // Legacy logs lacking a snapshot never silently show a changed meaning as an old answer.
    let legacy = attempts::Entity::find_by_id(uuid::Uuid::parse_str(&first).unwrap())
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut legacy: attempts::ActiveModel = legacy.into();
    legacy.snapshot = Set(None);
    legacy.update(app.db.as_ref()).await.unwrap();
    let list = history(&app, &app.owner_token, "").await;
    let legacy = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == first)
        .unwrap();
    assert!(legacy["card"].is_null());
    app.cleanup().await;
}
#[tokio::test]
async fn history_paginates_by_review_time_respects_dst_and_hides_other_users_and_revoked_notes() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    save(
        &app,
        session,
        &doc((0..4)
            .map(|i| entry(&format!("word{i}"), "nghĩa", &["Jobs"]))
            .collect()),
        0,
    )
    .await;
    let instants = [
        "2026-11-01T03:59:59Z",
        "2026-11-01T04:00:00Z",
        "2026-11-02T04:59:59Z",
        "2026-11-02T05:00:00Z",
    ];
    for (i, instant) in instants.iter().enumerate() {
        let id = start(&app, &app.owner_token, &format!("word{i}")).await;
        finish(&app, &app.owner_token, &id, 4).await;
        let row = attempts::Entity::find_by_id(uuid::Uuid::parse_str(&id).unwrap())
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        let mut row: attempts::ActiveModel = row.into();
        row.reviewed_at = Set(Some(chrono::DateTime::parse_from_rfc3339(instant).unwrap()));
        row.update(app.db.as_ref()).await.unwrap();
    }
    let page = history(&app, &app.owner_token, "?page=2&per_page=2").await;
    assert_eq!(page["meta"]["total"], 4);
    assert_eq!(page["meta"]["last_page"], 2);
    assert_eq!(page["data"].as_array().unwrap().len(), 2);
    let dst = history(
        &app,
        &app.owner_token,
        "?date=2026-11-01&timezone=America/New_York",
    )
    .await;
    assert_eq!(dst["meta"]["total"], 2);
    assert_eq!(dst["data"][0]["card"]["word"], "word2");
    assert_eq!(dst["data"][1]["card"]["word"], "word1");
    let share = app
        .api
        .post(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    share.assert_status(StatusCode::CREATED);
    let member = share.json::<Value>()["data"]["id"].as_i64().unwrap();
    assert_eq!(
        history(&app, &app.other_token, "").await["meta"]["total"],
        0
    );
    let id = start(&app, &app.other_token, "word0").await;
    finish(&app, &app.other_token, &id, 3).await;
    assert_eq!(
        history(&app, &app.other_token, "").await["meta"]["total"],
        1
    );
    assert_eq!(
        history(&app, &app.owner_token, "").await["meta"]["total"],
        4
    );
    app.api
        .delete(&format!(
            "/api/v1/sessions/{session}/collaborators/{member}"
        ))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        history(&app, &app.other_token, "").await["meta"]["total"],
        0
    );
    app.api
        .get(&format!("/api/v1/flashcards/history?session_id={session}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    for query in [
        "?page=0",
        "?per_page=101",
        "?rating=5",
        "?date=bad",
        "?timezone=bad",
        "?session_id=-1",
        "?extra=true",
    ] {
        app.api
            .get(&format!("/api/v1/flashcards/history{query}"))
            .add_header("authorization", auth(&app.owner_token))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .get("/api/v1/flashcards/history")
        .await
        .assert_status_unauthorized();
    app.api
        .delete(&format!("/api/v1/sessions/{session}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        history(&app, &app.owner_token, "").await["meta"]["total"],
        0
    );
    app.cleanup().await;
}
