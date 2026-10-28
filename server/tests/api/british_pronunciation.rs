use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::{json, Value};
use server::entities::{flashcard_memory, notes, vocabulary_cards};

async fn next(app: &LearningApp) -> Value {
    let response = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", format!("Bearer {}", app.owner_token))
        .json(&json!({}))
        .await;
    response.assert_status_ok();
    response.json::<Value>()["data"].clone()
}

#[tokio::test]
async fn british_backfill_preserves_source_note_and_existing_memory() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    let mut word = entry("history", "lịch sử", &[]);
    word["attrs"]["ipa"] = json!("/US legacy/");
    word["attrs"]["ipaUK"] = json!("/UK word/");
    word["attrs"]["ipaUS"] = json!("/US word/");
    word["content"][0]["content"][0]["attrs"]["ipa"] = json!("/US sentence/");
    word["content"][0]["content"][0]["attrs"]["ipaUK"] = json!("/UK sentence/");
    word["content"][0]["content"][0]["attrs"]["ipaUS"] = json!("/US sentence/");
    let content = doc(vec![word]);
    save(&app, session, &content, 0).await;
    let note_before = notes::Entity::find_by_id(session)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let auth = format!("Bearer {}", app.owner_token);
    app.api
        .put("/api/v1/flashcards")
        .add_header("authorization", auth.clone())
        .json(&json!({"direction":"en_vi","retention":0.9}))
        .await
        .assert_status_ok();
    let front = next(&app).await;
    assert_eq!(front["card"]["audio"]["locale"], "en-US");
    let ticket = front["card"]["id"].as_str().unwrap();
    let response = app
        .api
        .post(&format!("/api/v1/flashcards/{ticket}/reveal"))
        .add_header("authorization", auth.clone())
        .await;
    response.assert_status_ok();
    let back = response.json::<Value>()["data"]["back"].clone();
    assert_eq!(back["locale"], "en-US");
    assert_eq!(back["ipa"], "/US word/");
    assert_eq!(back["examples"][0]["ipa"], "/US sentence/");
    app.api
        .post(&format!("/api/v1/flashcards/{ticket}/grade"))
        .add_header("authorization", auth.clone())
        .json(&json!({"rating":4}))
        .await
        .assert_status_ok();
    let memory_before = flashcard_memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let american = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261028_010001_flashcard_american_ipa")
        .unwrap();
    let manager = migration::SchemaManager::new(app.db.as_ref());
    american.down(&manager).await.unwrap();
    american.up(&manager).await.unwrap();
    let rebuilt = vocabulary_cards::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rebuilt.ipa_us, "/US word/");
    // Emulate the previous projection, then exercise the production up/down/up migration.
    let migration = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261025_010001_british_pronunciation")
        .unwrap();
    let manager = migration::SchemaManager::new(app.db.as_ref());
    migration.down(&manager).await.unwrap();
    app.db.execute_unprepared("UPDATE vocabulary_cards SET examples = (SELECT jsonb_agg(value - 'ipaUK' - 'ipaUS') FROM jsonb_array_elements(examples))").await.unwrap();
    migration.up(&manager).await.unwrap();
    let card = vocabulary_cards::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(card.locale, "en-US");
    assert_eq!(card.ipa, "/US legacy/");
    assert_eq!(card.ipa_uk, "/UK word/");
    assert_eq!(card.examples[0]["ipaUK"], "/UK sentence/");
    assert_eq!(
        notes::Entity::find_by_id(session)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap(),
        note_before
    );
    assert_eq!(
        flashcard_memory::Entity::find()
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap(),
        memory_before
    );
    assert!(next(&app).await["card"].is_null());
    let library = app
        .api
        .get("/api/v1/vocabulary")
        .add_header("authorization", auth.clone())
        .await;
    library.assert_status_ok();
    let card = &library.json::<Value>()["data"][0];
    assert_eq!(card["locale"], "en-GB");
    assert_eq!(card["ipa"], "/UK word/");
    assert_eq!(card["examples"][0]["ipa"], "/UK sentence/");
    let response = app
        .api
        .post("/api/v1/vocabulary-reviews")
        .add_header("authorization", auth.clone())
        .json(&json!({"mode":"typing","limit":1}))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    let review_id = response.json::<Value>()["data"]["id"].as_i64().unwrap();
    let response = app
        .api
        .post(&format!("/api/v1/vocabulary-reviews/{review_id}/answers"))
        .add_header("authorization", auth)
        .json(&json!({"position":0,"answer":"history"}))
        .await;
    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>()["data"]["result"]["card"]["ipa"],
        "/UK word/"
    );
    assert_eq!(
        response.json::<Value>()["data"]["result"]["card"]["locale"],
        "en-GB"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn us_only_ipa_is_not_presented_as_uk_and_new_review_uses_british_voice() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    save(
        &app,
        session,
        &doc(vec![entry("teacher", "giáo viên", &[])]),
        0,
    )
    .await;
    let auth = format!("Bearer {}", app.owner_token);
    let front = next(&app).await;
    let ticket = front["card"]["id"].as_str().unwrap();
    let response = app
        .api
        .post(&format!("/api/v1/flashcards/{ticket}/reveal"))
        .add_header("authorization", auth.clone())
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["back"]["ipa"], "/tiːtʃə/");
    assert_eq!(response.json::<Value>()["data"]["back"]["locale"], "en-US");
    let response = app
        .api
        .post("/api/v1/vocabulary-reviews")
        .add_header("authorization", auth)
        .json(&json!({"mode":"typing","limit":1}))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    assert_eq!(
        response.json::<Value>()["data"]["question"]["locale"],
        "en-GB"
    );
    assert_eq!(response.json::<Value>()["data"]["question"]["ipa"], "");
    app.cleanup().await;
}
