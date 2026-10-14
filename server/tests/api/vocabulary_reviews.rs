use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, library, save},
};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use server::entities::{vocabulary_cards as cards, vocabulary_progress as progress};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn seed(app: &LearningApp) -> (i64, Value) {
    let id = lesson(app, &app.owner_token).await;
    let content = doc(vec![
        entry("teacher", "giáo viên", &["Jobs"]),
        entry("chef", "đầu bếp", &["Jobs"]),
        entry("doctor", "bác sĩ", &["Jobs"]),
    ]);
    save(app, id, &content, 0).await;
    (id, content)
}
async fn start(app: &LearningApp, token: &str, mode: &str, q: &str) -> (i64, Value) {
    let r = app
        .api
        .post("/api/v1/vocabulary-reviews")
        .add_header("authorization", auth(token))
        .json(&json!({"mode":mode,"q":q,"limit":1}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let value = r.json::<Value>()["data"].clone();
    (value["id"].as_i64().unwrap(), value)
}
async fn answer(app: &LearningApp, token: &str, id: i64, word: &str, attempt: u8) -> Value {
    let r = app
        .api
        .post(&format!("/api/v1/vocabulary-reviews/{id}/answers"))
        .add_header("authorization", auth(token))
        .json(&json!({"position":0,"answer":word,"attempt":attempt}))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn typing_two_attempts_persist_retry_without_answer_key_and_replay_counts_once() {
    let app = LearningApp::spawn().await;
    seed(&app).await;
    let (id, first) = start(&app, &app.owner_token, "typing", "teacher").await;
    assert!(first["question"].get("word").is_none());
    assert!(first["question"].get("card").is_none());
    assert!(first["question"].get("source_sessions").is_none());
    assert_eq!(first["question"]["letters"].as_array().unwrap().len(), 7);
    assert_eq!(first["question"]["attempt"], 0);
    app.api
        .get(&format!("/api/v1/vocabulary-reviews/{id}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    let audio = app
        .api
        .get(&format!(
            "/api/v1/vocabulary-reviews/{id}/pronunciation?position=0"
        ))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    audio.assert_status_ok();
    assert_eq!(audio.json::<Value>()["data"]["text"], "teacher");
    let wrong = answer(&app, &app.owner_token, id, "doctor", 0).await;
    assert!(wrong["result"].is_null());
    assert_eq!(wrong["review"]["question"]["attempt"], 1);
    let retry = app
        .api
        .get(&format!("/api/v1/vocabulary-reviews/{id}"))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    retry.assert_status_ok();
    assert!(
        retry.json::<Value>()["data"]["question"]["retry"]["pattern"]
            .as_array()
            .unwrap()
            .iter()
            .any(Value::is_null)
    );
    let repeat = answer(&app, &app.owner_token, id, " doctor ", 0).await;
    assert!(repeat["result"].is_null());
    let correct = answer(&app, &app.owner_token, id, " Teacher ", 1).await;
    assert_eq!(correct["result"]["correct"], true);
    assert_eq!(correct["result"]["assisted"], true);
    assert_eq!(correct["result"]["card"]["word"], "teacher");
    assert!(!correct["result"]["card"]["ipa"]
        .as_str()
        .unwrap()
        .is_empty());
    assert_eq!(correct["review"]["complete"], true);
    assert_eq!(correct["review"]["mistakes"].as_array().unwrap().len(), 1);
    answer(&app, &app.owner_token, id, "teacher", 1).await;
    let rows = progress::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].correct, rows[0].incorrect, rows[0].streak),
        (1, 1, 0)
    );
    app.api
        .post(&format!("/api/v1/vocabulary-reviews/{id}/answers"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"position":0,"answer":"chef","attempt":1}))
        .await
        .assert_status(StatusCode::CONFLICT);
    let lib = library(&app, &app.owner_token, "?q=teacher").await;
    assert_eq!(lib["data"][0]["progress"]["incorrect"], 1);
    let (_, priority) = start(&app, &app.owner_token, "typing", "").await;
    assert_eq!(priority["question"]["meaning"], "giáo viên");
    app.cleanup().await;
}
#[tokio::test]
async fn choice_rejects_unknown_options_and_finishes_on_second_wrong_attempt() {
    let app = LearningApp::spawn().await;
    seed(&app).await;
    let (id, review) = start(&app, &app.owner_token, "choice", "teacher").await;
    let options = review["question"]["options"].as_array().unwrap();
    assert!(options.iter().any(|o| o == "teacher"));
    assert_eq!(options.len(), 3);
    assert!(review["question"].get("ipa").is_none());
    app.api
        .post(&format!("/api/v1/vocabulary-reviews/{id}/answers"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"position":0,"answer":"nonsense"}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let first = answer(&app, &app.owner_token, id, "chef", 0).await;
    assert!(first["result"].is_null());
    let second = answer(&app, &app.owner_token, id, "doctor", 1).await;
    assert_eq!(second["result"]["correct"], false);
    assert_eq!(second["result"]["card"]["word"], "teacher");
    assert_eq!(second["review"]["answered"], 1);
    let rows = progress::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!((rows[0].correct, rows[0].incorrect), (0, 1));
    app.cleanup().await;
}
#[tokio::test]
async fn concurrent_identical_answers_are_idempotent_and_progress_is_per_user() {
    let app = LearningApp::spawn().await;
    let (session, _) = seed(&app).await;
    let (id, _) = start(&app, &app.owner_token, "choice", "teacher").await;
    let a = answer(&app, &app.owner_token, id, "teacher", 0);
    let b = answer(&app, &app.owner_token, id, "teacher", 0);
    let (a, b) = tokio::join!(a, b);
    assert_eq!(a["result"], b["result"]);
    let rows = progress::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].correct, 1);
    assert_eq!(rows[0].streak, 1);
    app.api
        .post(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await
        .assert_status(StatusCode::CREATED);
    assert_eq!(
        library(&app, &app.other_token, "?q=teacher").await["data"][0]["progress"]["correct"],
        0
    );
    let (other, _) = start(&app, &app.other_token, "typing", "teacher").await;
    answer(&app, &app.other_token, other, "teacher", 0).await;
    assert_eq!(
        progress::Entity::find()
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        2
    );
    app.cleanup().await;
}
#[tokio::test]
async fn changed_word_blocks_pending_review_and_reset_progress_but_ipa_edits_do_not() {
    let app = LearningApp::spawn().await;
    let (id, mut content) = seed(&app).await;
    let (first, _) = start(&app, &app.owner_token, "typing", "teacher").await;
    content["content"][0]["content"][0]["content"][0]["attrs"]["ipa"] = json!("/new-ipa/");
    save(&app, id, &content, 1).await;
    let result = answer(&app, &app.owner_token, first, "teacher", 0).await;
    assert_eq!(result["result"]["card"]["ipa"], "/new-ipa/");
    let (pending, _) = start(&app, &app.owner_token, "typing", "teacher").await;
    content["content"][0]["content"][0]["content"][0]["content"][0]["attrs"]["meaning"] =
        json!("người giảng dạy");
    save(&app, id, &content, 2).await;
    app.api
        .get(&format!("/api/v1/vocabulary-reviews/{pending}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::CONFLICT);
    assert_eq!(
        library(&app, &app.owner_token, "?q=teacher").await["data"][0]["progress"]["correct"],
        0
    );
    let (fresh, _) = start(&app, &app.owner_token, "typing", "teacher").await;
    answer(&app, &app.owner_token, fresh, "teacher", 0).await;
    let card = cards::Entity::find()
        .filter(cards::Column::Word.eq("teacher"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        progress::Entity::find_by_id((app.owner_id, card.id))
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .correct,
        1
    );
    app.cleanup().await;
}
#[tokio::test]
async fn reviews_hide_revoked_and_deleted_sources_and_validate_empty_sets() {
    let app = LearningApp::spawn().await;
    let (session, _) = seed(&app).await;
    app.api
        .post(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await
        .assert_status(StatusCode::CREATED);
    let (id, _) = start(&app, &app.other_token, "typing", "teacher").await;
    let member = library(&app, &app.other_token, "").await;
    assert_eq!(member["data"][0]["source"]["shared"], true);
    let members = app
        .api
        .get(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    let user = members["data"][0]["id"].as_i64().unwrap();
    app.api
        .delete(&format!("/api/v1/sessions/{session}/collaborators/{user}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .get(&format!("/api/v1/vocabulary-reviews/{id}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .post(&format!("/api/v1/vocabulary-reviews/{id}/answers"))
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"position":0,"answer":"teacher"}))
        .await
        .assert_status_not_found();
    for input in [
        json!({"mode":"invalid"}),
        json!({"mode":"typing","limit":0}),
        json!({"mode":"typing","q":"does not exist"}),
    ] {
        app.api
            .post("/api/v1/vocabulary-reviews")
            .add_header("authorization", auth(&app.owner_token))
            .json(&input)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (owner, _) = start(&app, &app.owner_token, "typing", "chef").await;
    app.api
        .delete(&format!("/api/v1/sessions/{session}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .get(&format!("/api/v1/vocabulary-reviews/{owner}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
