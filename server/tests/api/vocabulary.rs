use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::{notes, vocabulary_cards as cards};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
pub(super) async fn lesson(app: &LearningApp, token: &str) -> i64 {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", auth(token))
        .json(&json!({"name":"Vocabulary school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let p = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app.api.post(&format!("/api/v1/places/{p}/sessions")).add_header("authorization",auth(token)).json(&json!({"title":"Jobs","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"Asia/Saigon"})).await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
pub(super) fn entry(word: &str, meaning: &str, topics: &[&str]) -> Value {
    json!({"type":"vocabularyEntry","attrs":{"id":uuid::Uuid::new_v4().to_string(),"word":word,"ipa":" /tiːtʃə/ ","topics":topics},"content":[{"type":"vocabularySense","attrs":{"id":uuid::Uuid::new_v4().to_string(),"partOfSpeech":"noun","meaning":meaning},"content":[{"type":"vocabularyExample","attrs":{"id":uuid::Uuid::new_v4().to_string(),"text":" My father is a teacher. ","ipa":"","meaning":" Bố tôi là giáo viên. ","highlights":[],"meaningHighlights":[]}}]}]})
}
pub(super) fn doc(entries: Vec<Value>) -> Value {
    json!({"type":"doc","content":[{"type":"blockquote","content":[{"type":"vocabulary","attrs":{"locale":"en-US"},"content":entries}]}]})
}
pub(super) async fn save(app: &LearningApp, id: i64, content: &Value, revision: i32) {
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"content":content,"schema_version":3,"base_revision":revision}))
        .await
        .assert_status_ok();
}
pub(super) async fn library(app: &LearningApp, token: &str, query: &str) -> Value {
    let r = app
        .api
        .get(&format!("/api/v1/vocabulary{query}"))
        .add_header("authorization", auth(token))
        .await;
    r.assert_status_ok();
    r.json()
}
#[tokio::test]
async fn vocabulary_syncs_same_transaction_updates_without_duplicates_and_reactivates_on_undo() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app, &app.owner_token).await;
    let mut content = doc(vec![entry(
        "  a teacher  ",
        " giáo viên ",
        &["Jobs", " Jobs "],
    )]);
    save(&app, id, &content, 0).await;
    let first = library(&app, &app.owner_token, "").await;
    assert_eq!(first["meta"]["total"], 1);
    assert_eq!(first["data"][0]["word"], "a teacher");
    assert_eq!(first["data"][0]["meaning"], "giáo viên");
    assert_eq!(first["data"][0]["topics"], json!(["Jobs"]));
    assert_eq!(
        first["data"][0]["examples"][0]["text"],
        "My father is a teacher."
    );
    let card = first["data"][0]["id"].as_i64().unwrap();
    let before = cards::Entity::find_by_id(card)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    save(&app, id, &content, 1).await;
    let same = cards::Entity::find_by_id(card)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.updated_at, same.updated_at);
    content["content"][0]["content"][0]["content"][0]["content"][0]["attrs"]["meaning"] =
        json!(" giáo viên tiếng Anh ");
    save(&app, id, &content, 2).await;
    let changed = library(&app, &app.owner_token, "").await;
    assert_eq!(changed["data"][0]["id"], card);
    assert_eq!(changed["data"][0]["meaning"], "giáo viên tiếng Anh");
    let empty = json!({"type":"doc","content":[{"type":"paragraph"}]});
    save(&app, id, &empty, 3).await;
    assert_eq!(
        library(&app, &app.owner_token, "").await["meta"]["total"],
        0
    );
    assert!(
        !cards::Entity::find_by_id(card)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .active
    );
    save(&app, id, &content, 4).await;
    assert_eq!(
        library(&app, &app.owner_token, "").await["data"][0]["id"],
        card
    );
    // A conflicting write must not change either document or its derived vocabulary.
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"content":empty,"schema_version":3,"base_revision":0}))
        .await
        .assert_status(StatusCode::CONFLICT);
    assert_eq!(
        library(&app, &app.owner_token, "").await["meta"]["total"],
        1
    );
    app.cleanup().await;
}
#[tokio::test]
async fn vocabulary_filters_paginate_large_tables_and_hide_other_owners_and_revoked_shares() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app, &app.owner_token).await;
    let entries: Vec<_> = (0..130)
        .map(|i| {
            entry(
                &format!("teacher {i:03}"),
                "giáo viên",
                &[if i % 2 == 0 { "Jobs" } else { "Travel" }],
            )
        })
        .chain([entry("100%_match", "ký hiệu", &["Jobs"])])
        .collect();
    save(&app, id, &doc(entries), 0).await;
    let p1 = library(&app, &app.owner_token, "").await;
    assert_eq!(p1["data"].as_array().unwrap().len(), 50);
    assert_eq!(p1["meta"]["total"], 131);
    assert_eq!(p1["meta"]["last_page"], 3);
    assert_eq!(p1["meta"]["current_page"], 1);
    assert_eq!(p1["meta"]["per_page"], 50);
    assert!(p1["meta"].get("page").is_none());
    let small = library(&app, &app.owner_token, "?per_page=10&page=2").await;
    assert_eq!(small["data"].as_array().unwrap().len(), 10);
    assert_eq!(small["meta"]["per_page"], 10);
    assert_eq!(small["meta"]["current_page"], 2);
    assert_eq!(p1["meta"]["topics"], json!(["Jobs", "Travel"]));
    let p3 = library(&app, &app.owner_token, "?page=3").await;
    assert_eq!(p3["data"].as_array().unwrap().len(), 31);
    assert_eq!(
        library(&app, &app.owner_token, "?topic=Jobs").await["meta"]["total"],
        66
    );
    assert_eq!(
        library(&app, &app.owner_token, "?q=%25_").await["meta"]["total"],
        1
    );
    assert_eq!(
        library(&app, &app.other_token, "?q=teacher").await["meta"]["total"],
        0
    );
    app.api
        .get(&format!("/api/v1/vocabulary?session_id={id}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{id}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let member = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let shared = library(
        &app,
        &app.other_token,
        &format!("?session_id={id}&topic=Jobs"),
    )
    .await;
    assert_eq!(shared["meta"]["total"], 66);
    assert_eq!(shared["meta"]["source"]["shared"], true);
    assert_eq!(shared["data"][0]["source"]["shared"], true);
    app.api
        .delete(&format!("/api/v1/sessions/{id}/collaborators/{member}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        library(&app, &app.other_token, "").await["meta"]["total"],
        0
    );
    app.api
        .delete(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        library(&app, &app.owner_token, "").await["meta"]["total"],
        0
    );
    app.cleanup().await;
}
#[tokio::test]
async fn vocabulary_syncs_realtime_snapshots_and_backfills_old_notes_idempotently() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let id = lesson(&app, &app.owner_token).await;
    let content = doc(vec![entry("teacher", "giáo viên", &["Jobs"])]);
    let now = chrono::Utc::now().fixed_offset();
    notes::ActiveModel {
        session_id: Set(id),
        content: Set(content.clone()),
        schema_version: Set(3),
        revision: Set(1),
        yjs_state: Set(None),
        saved_at: Set(now),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    assert_eq!(server::rebuild_vocabulary(&app.db).await.unwrap(), 1);
    let initial = library(&app, &app.owner_token, "").await;
    server::rebuild_vocabulary(&app.db).await.unwrap();
    assert_eq!(
        library(&app, &app.owner_token, "").await["data"][0]["id"],
        initial["data"][0]["id"]
    );
    app.api.put(&format!("/api/v1/internal/notes/{id}")).add_header("x-collaboration-secret","test-collaboration-secret-32-characters").json(&json!({"state":"AAA=","content":doc(vec![entry("chef","đầu bếp",&["Jobs"])]),"base_revision":1})).await.assert_status_ok();
    let updated = library(&app, &app.owner_token, "").await;
    assert_eq!(updated["meta"]["total"], 1);
    assert_eq!(updated["data"][0]["word"], "chef");
    app.cleanup().await;
}
#[tokio::test]
async fn vocabulary_validates_filters_and_counts_words_separately_from_senses() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app, &app.owner_token).await;
    let word = entry("Teacher", "giáo viên", &["Jobs"]);
    let second = entry("  TEACHER ", "người dạy", &["Jobs"]);
    save(
        &app,
        id,
        &doc(vec![word, second, entry("", "", &["Jobs"])]),
        0,
    )
    .await;
    let lib = library(&app, &app.owner_token, "").await;
    assert_eq!(lib["meta"]["word_count"], 1);
    assert_eq!(lib["meta"]["total"], 2);
    assert_eq!(lib["meta"]["reviewable"], 2);
    app.api
        .get("/api/v1/vocabulary")
        .await
        .assert_status_unauthorized();
    for query in ["?page=0", "?session_id=-1", "?extra=oops"] {
        app.api
            .get(&format!("/api/v1/vocabulary{query}"))
            .add_header("authorization", auth(&app.owner_token))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        cards::Entity::find()
            .filter(cards::Column::SessionId.eq(id))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        2
    );
    app.cleanup().await;
}
