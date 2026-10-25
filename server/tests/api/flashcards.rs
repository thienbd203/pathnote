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
fn typed_entry(word: &str, meaning: &str, pos: &str) -> Value {
    let mut value = entry(word, meaning, &[]);
    value["content"][0]["attrs"]["partOfSpeech"] = json!(pos);
    value
}

#[tokio::test]
async fn grouping_migration_preserves_latest_schedule_and_reconstructs_its_original_chain() {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectionTrait, DbBackend, QueryOrder, Statement};
    use server::entities::vocabulary_cards as vocabulary;
    use sha2::{Digest, Sha256};
    let app = LearningApp::spawn_before_flashcard_grouping().await;
    // Current handlers need the later projection column while this test seeds the old grouping schema.
    app.db
        .execute_unprepared(
            "ALTER TABLE vocabulary_cards ADD COLUMN ipa_uk TEXT NOT NULL DEFAULT ''",
        )
        .await
        .unwrap();
    let a = lesson(&app, &app.owner_token).await;
    let b = lesson(&app, &app.owner_token).await;
    save(
        &app,
        a,
        &doc(vec![typed_entry("look\u{a0}up", "tra cứu", "verb")]),
        0,
    )
    .await;
    save(
        &app,
        b,
        &doc(vec![typed_entry("Look   up", "tìm kiếm", "verb")]),
        0,
    )
    .await;
    let cards = vocabulary::Entity::find()
        .order_by_asc(vocabulary::Column::Id)
        .all(app.db.as_ref())
        .await
        .unwrap();
    let now = Utc::now().fixed_offset();
    let due = now + Duration::days(5);
    for (index, card) in cards.iter().enumerate() {
        let identity = json!([card.word, card.meaning, card.locale, card.part_of_speech]);
        let key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&identity).unwrap())
        );
        memory::ActiveModel {
            user_id: Set(app.owner_id),
            card_id: Set(card.id),
            direction: Set("vi_en".into()),
            identity: Set(identity),
            study_key: Set(key.clone()),
            parameter_version: Set(0),
            stability: Set(2.0),
            difficulty: Set(5.0),
            step: Set(0),
            reviews: Set(2),
            lapses: Set(0),
            due_at: Set(due + Duration::days(index as i64)),
            reviewed_at: Set(now - Duration::days(2 - index as i64)),
        }
        .insert(app.db.as_ref())
        .await
        .unwrap();
        for version in 0..2 {
            let at = now - Duration::days(6 - index as i64 - version as i64 * 4);
            app.db.execute(Statement::from_sql_and_values(DbBackend::Postgres,
                "INSERT INTO flashcard_attempts (id,user_id,card_id,direction,study_key,version,retention,revealed,rating,due_at,reviewed_at,created_at,parameter_version,delta_days) VALUES ($1,$2,$3,'vi_en',$4,$5,0.9,true,3,$6,$7,$7,0,$8)",
                [uuid::Uuid::new_v4().into(), app.owner_id.into(), card.id.into(), key.clone().into(), version.into(), due.into(), at.into(), (version * 4).into()]
            )).await.unwrap();
        }
    }
    // An old source may have changed after the last review, while its memory
    // must still apply to the unchanged duplicate in the first lesson.
    save(
        &app,
        b,
        &doc(vec![typed_entry("recall", "gợi nhớ", "verb")]),
        1,
    )
    .await;
    let old = memory::Entity::find()
        .filter(memory::Column::CardId.eq(cards[1].id))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    app.db
        .execute_unprepared("ALTER TABLE vocabulary_cards DROP COLUMN ipa_uk")
        .await
        .unwrap();
    Migrator::up(app.db.as_ref(), None).await.unwrap();
    let rows = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].due_at, old.due_at);
    assert_eq!(rows[0].reviews, old.reviews);
    assert_eq!(rows[0].stability, old.stability);
    let result = next(&app, &app.owner_token, json!({"session_id":a})).await;
    assert!(result["card"].is_null());
    assert_eq!(result["overview"]["new"], 0);
    let mut row: memory::ActiveModel = rows[0].clone().into();
    row.parameter_version = Set(-1); // Force reconstruction using the retained history chain.
    row.due_at = Set(now - Duration::minutes(1));
    row.update(app.db.as_ref()).await.unwrap();
    let result = next(&app, &app.owner_token, json!({"session_id":a})).await;
    assert_eq!(result["card"]["new"], false);
    let ticket = result["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 4).await;
    let updated = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.reviews, 3);
    assert_eq!(updated.card_id, old.card_id);
    app.cleanup().await;
}

#[tokio::test]
async fn repeated_words_share_memory_and_combine_meanings_across_lessons() {
    let app = LearningApp::spawn().await;
    let first = lesson(&app, &app.owner_token).await;
    let second = lesson(&app, &app.owner_token).await;
    let private = lesson(&app, &app.other_token).await;
    app.api.put(&format!("/api/v1/sessions/{private}/note"))
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"content":doc(vec![typed_entry("remember", "nghĩa riêng tư", "verb")]),"schema_version":4,"base_revision":0}))
        .await.assert_status_ok();
    save(
        &app,
        first,
        &doc(vec![typed_entry("remember", "nhớ", "verb")]),
        0,
    )
    .await;
    save(
        &app,
        second,
        &doc(vec![typed_entry("Remember", "ghi nhớ", "verb")]),
        0,
    )
    .await;
    let a = next(&app, &app.owner_token, json!({"session_id":first})).await;
    assert_eq!(a["overview"]["new"], 1);
    assert_eq!(a["card"]["front"], "ghi nhớ; nhớ");
    let ticket = a["card"]["id"].as_str().unwrap();
    assert_eq!(
        reveal(&app, &app.owner_token, ticket).await["back"]["meaning"],
        "ghi nhớ; nhớ"
    );
    let result = grade(&app, ticket, 4).await;
    let history = app
        .api
        .get("/api/v1/flashcards/history")
        .add_header("authorization", auth(&app.owner_token))
        .await;
    history.assert_status_ok();
    assert_eq!(
        history.json::<Value>()["data"][0]["card"]["meaning"],
        "ghi nhớ; nhớ"
    );
    let private = next(&app, &app.other_token, json!({})).await;
    assert_eq!(private["overview"]["new"], 1);
    assert_eq!(private["card"]["front"], "nghĩa riêng tư");
    let next_other_lesson = next(&app, &app.owner_token, json!({"session_id":second})).await;
    assert!(next_other_lesson["card"].is_null());
    assert_eq!(next_other_lesson["overview"]["new"], 0);
    let all = next(&app, &app.owner_token, json!({})).await;
    assert_eq!(all["overview"]["reviewable"], 1);
    assert_eq!(all["overview"]["word_count"], 1);
    assert_eq!(all["overview"]["next_due_at"], result["due_at"]);
    // Importing another wording does not reset the common schedule.
    let third = lesson(&app, &app.owner_token).await;
    save(
        &app,
        third,
        &doc(vec![typed_entry("remember", "nhớ lại", "verb")]),
        0,
    )
    .await;
    assert!(next(&app, &app.owner_token, json!({"session_id":third})).await["card"].is_null());
    // A removed source must not delete memory still used by another lesson.
    app.api
        .delete(&format!("/api/v1/sessions/{first}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let row = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(json!(row.due_at), result["due_at"]);
    let mut row: memory::ActiveModel = row.into();
    row.due_at = Set(Utc::now().fixed_offset() - Duration::minutes(1));
    row.update(app.db.as_ref()).await.unwrap();
    let due = next(&app, &app.owner_token, json!({"session_id":second})).await;
    assert_eq!(due["card"]["new"], false);
    let ticket = due["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 4).await;
    let rows = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reviews, 2);
    app.cleanup().await;
}

#[tokio::test]
async fn same_spelling_with_different_parts_of_speech_or_languages_stays_separate() {
    let app = LearningApp::spawn().await;
    let first = lesson(&app, &app.owner_token).await;
    let second = lesson(&app, &app.owner_token).await;
    save(
        &app,
        first,
        &doc(vec![
            typed_entry("light", "ánh sáng", "noun"),
            typed_entry("light", "nhẹ", "adjective"),
        ]),
        0,
    )
    .await;
    save(
        &app,
        second,
        &doc(vec![typed_entry("Light", "nguồn sáng", "noun")]),
        0,
    )
    .await;
    let all = next(&app, &app.owner_token, json!({})).await;
    assert_eq!(all["overview"]["new"], 2);
    assert_eq!(all["overview"]["reviewable"], 2);
    assert_eq!(all["overview"]["word_count"], 1);
    let mut other_language = doc(vec![typed_entry("light", "ánh sáng", "noun")]);
    other_language["content"][0]["content"][0]["attrs"]["locale"] = json!("en-GB");
    let third = lesson(&app, &app.owner_token).await;
    save(&app, third, &other_language, 0).await;
    assert_eq!(
        next(&app, &app.owner_token, json!({})).await["overview"]["new"],
        3
    );
    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_tickets_from_different_lessons_cannot_double_grade_the_common_memory() {
    let app = LearningApp::spawn().await;
    let a = lesson(&app, &app.owner_token).await;
    let b = lesson(&app, &app.owner_token).await;
    for id in [a, b] {
        save(
            &app,
            id,
            &doc(vec![typed_entry("remember", "nhớ", "verb")]),
            0,
        )
        .await;
    }
    let a = next(&app, &app.owner_token, json!({"session_id":a})).await;
    let b = next(&app, &app.owner_token, json!({"session_id":b})).await;
    let a = a["card"]["id"].as_str().unwrap();
    let b = b["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, a).await;
    reveal(&app, &app.owner_token, b).await;
    let submit = |id: &str| {
        app.api
            .post(&format!("/api/v1/flashcards/{id}/grade"))
            .add_header("authorization", auth(&app.owner_token))
            .json(&json!({"rating":4}))
    };
    let (first, second) = tokio::join!(submit(a), submit(b));
    assert!([first.status_code(), second.status_code()].contains(&StatusCode::OK));
    assert!([first.status_code(), second.status_code()].contains(&StatusCode::CONFLICT));
    assert_eq!(
        memory::Entity::find()
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .reviews,
        1
    );
    app.cleanup().await;
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
    // A later retry must return the exact persisted result too, regardless of
    // which concurrent request originally performed the write.
    assert_eq!(a, grade(&app, ticket, 4).await);
    let rows = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reviews, 1);
    assert_eq!(rows[0].stability, expected.memory.stability);
    assert_eq!(rows[0].difficulty, expected.memory.difficulty);
    assert_eq!(a["due_at"], json!(rows[0].due_at));
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
    content["content"][0]["content"][0]["content"][0]["attrs"]["ipaUK"] = json!("/new/");
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
    // Rewording a meaning keeps the shared word schedule, while the old
    // content ticket above is still rejected.
    assert!(new["card"].is_null());
    assert_eq!(new["overview"]["new"], 0);
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

#[tokio::test]
async fn runs_freeze_configuration_group_history_and_finish_without_double_counting() {
    let app = LearningApp::spawn().await;
    let (session, _) = seed(&app).await;
    let filters = json!({"session_id":session});
    settings(&app, filters.clone(), "both", 0.9).await;
    let response = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&filters)
        .await;
    response.assert_status(StatusCode::CREATED);
    let run = response.json::<Value>()["data"].clone();
    assert_eq!(run["initial_total"], 2);
    let id = run["id"].as_str().unwrap();
    settings(&app, filters.clone(), "vi_en", 0.8).await;
    for _ in 0..2 {
        let card = next(&app, &app.owner_token, json!({"run_id":id})).await;
        assert_eq!(card["overview"]["settings"]["direction"], "both");
        let ticket = card["card"]["id"].as_str().unwrap();
        reveal(&app, &app.owner_token, ticket).await;
        grade(&app, ticket, 4).await;
        grade(&app, ticket, 4).await;
    }
    assert!(next(&app, &app.owner_token, json!({"run_id":id})).await["card"].is_null());
    let path = format!("/api/v1/flashcards/runs/{id}");
    let response = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await;
    response.assert_status_ok();
    let summary = response.json::<Value>()["data"].clone();
    assert_eq!(summary["reviewed"], 2);
    assert_eq!(summary["unique_reviewed"], 2);
    assert_eq!(summary["ratings"], json!([0, 0, 0, 2]));
    assert!(summary["finished_at"].is_string());
    app.api
        .get(&path)
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .post(&format!("{path}/finish"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    let response = app
        .api
        .get(&format!("/api/v1/flashcards/history?run_id={id}"))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 2);
    app.api
        .get(&format!("/api/v1/flashcards/history?run_id={id}"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status_not_found();
    let today = chrono::Utc::now().date_naive();
    let response = app
        .api
        .get(&format!(
            "/api/v1/flashcards/history?from={today}&to={today}"
        ))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 2);
    app.api
        .get("/api/v1/flashcards/history?from=2026-10-15&to=2026-10-01")
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.cleanup().await;
}

#[tokio::test]
async fn new_only_excludes_due_cards_and_closed_runs_reject_ungraded_tickets() {
    let app = LearningApp::spawn().await;
    let (session, _) = seed(&app).await;
    let first = next(&app, &app.owner_token, json!({"session_id":session})).await;
    let ticket = first["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 1).await;
    memory::Entity::update_many()
        .col_expr(
            memory::Column::DueAt,
            sea_orm::sea_query::Expr::value(Utc::now().fixed_offset() - Duration::minutes(1)),
        )
        .exec(app.db.as_ref())
        .await
        .unwrap();
    assert!(next(
        &app,
        &app.owner_token,
        json!({"session_id":session,"new_only":true})
    )
    .await["card"]
        .is_null());
    let response = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"session_id":session}))
        .await;
    let run = response.json::<Value>()["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pending = next(&app, &app.owner_token, json!({"run_id":run})).await;
    let ticket = pending["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    app.api
        .post(&format!("/api/v1/flashcards/runs/{run}/finish"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status_ok();
    app.api
        .post(&format!("/api/v1/flashcards/{ticket}/grade"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"rating":3}))
        .await
        .assert_status(StatusCode::CONFLICT);
    app.cleanup().await;
}

#[tokio::test]
async fn next_query_profile() {
    if std::env::var_os("PATHNOTE_SQL_PROFILE").is_some() {
        let _ = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter("sqlx::query=info")
            .try_init();
    }
    let app = LearningApp::spawn().await;
    let (id, _) = seed(&app).await;
    let f = json!({"session_id": id});
    eprintln!("PROFILE_SINGLE_BEGIN");
    let first = next(&app, &app.owner_token, f.clone()).await;
    eprintln!("PROFILE_SINGLE_END");
    assert!(!first["card"].is_null());
    settings(&app, f.clone(), "both", 0.9).await;
    eprintln!("PROFILE_BOTH_BEGIN");
    next(&app, &app.owner_token, f.clone()).await;
    eprintln!("PROFILE_BOTH_END");
    let run = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&f)
        .await;
    run.assert_status(StatusCode::CREATED);
    eprintln!("PROFILE_RUN_BEGIN");
    next(
        &app,
        &app.owner_token,
        json!({"run_id": run.json::<Value>()["data"]["id"]}),
    )
    .await;
    eprintln!("PROFILE_RUN_END");
    app.cleanup().await;
}

#[tokio::test]
async fn overview_exposes_counts_without_returning_the_vocabulary_library() {
    let app = LearningApp::spawn().await;
    let (id, _) = seed(&app).await;
    let response = app
        .api
        .get("/api/v1/flashcards")
        .add_header("authorization", auth(&app.owner_token))
        .await;
    response.assert_status_ok();
    let data = response.json::<Value>()["data"].clone();
    assert_eq!(data["word_count"], 1);
    assert_eq!(data["reviewable"], 1);
    assert_eq!(data["new"], 1);
    assert!(data.get("cards").is_none());
    assert!(data.get("entries").is_none());
    settings(&app, json!({}), "both", 0.9).await;
    let response = app
        .api
        .get("/api/v1/flashcards")
        .add_header("authorization", auth(&app.owner_token))
        .await;
    let data = response.json::<Value>()["data"].clone();
    assert_eq!(data["new"], 2);
    assert_eq!(data["word_count"], 1);
    assert_eq!(data["reviewable"], 1);
    let other = app
        .api
        .get("/api/v1/flashcards")
        .add_header("authorization", auth(&app.other_token))
        .await;
    assert_eq!(other.json::<Value>()["data"]["reviewable"], 0);
    assert!(id > 0);
    app.cleanup().await;
}

#[tokio::test]
async fn meanings_index_migration_can_be_reversed_without_touching_schedules() {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let app = LearningApp::spawn().await;
    async fn exists(db: &sea_orm::DatabaseConnection, name: &str) -> bool {
        let row = db.query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE schemaname = current_schema() AND indexname = $1) AS present",
            [name.into()],
        )).await.unwrap().unwrap();
        row.try_get("", "present").unwrap()
    }
    assert!(exists(&app.db, "vocabulary_cards_word_locale_pos").await);
    assert!(!exists(&app.db, "flashcard_memory_identity").await);
    let steps = Migrator::migrations()
        .into_iter()
        .rev()
        .position(|m| m.name() == "m20261016_030001_flashcard_meanings_index")
        .expect("meanings index migration is registered") as u32
        + 1;
    Migrator::down(app.db.as_ref(), Some(steps)).await.unwrap();
    assert!(!exists(&app.db, "vocabulary_cards_word_locale_pos").await);
    assert!(exists(&app.db, "flashcard_memory_identity").await);
    Migrator::up(app.db.as_ref(), None).await.unwrap();
    assert!(exists(&app.db, "vocabulary_cards_word_locale_pos").await);
    app.cleanup().await;
}

#[tokio::test]
async fn run_counts_shared_word_once_even_when_retried_from_another_lesson() {
    use server::entities::{flashcard_attempts as attempts, vocabulary_cards as cards};
    let app = LearningApp::spawn().await;
    for meaning in ["nhớ", "ghi nhớ"] {
        let session = lesson(&app, &app.owner_token).await;
        save(
            &app,
            session,
            &doc(vec![typed_entry("remember", meaning, "verb")]),
            0,
        )
        .await;
    }
    let response = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({}))
        .await;
    response.assert_status(StatusCode::CREATED);
    let run = response.json::<Value>()["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let first = next(&app, &app.owner_token, json!({"run_id":run})).await;
    let ticket = first["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 1).await;
    let attempt = attempts::Entity::find_by_id(uuid::Uuid::parse_str(ticket).unwrap())
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let source = cards::Entity::find_by_id(attempt.card_id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    save(
        &app,
        source.session_id,
        &json!({"type":"doc","content":[{"type":"paragraph"}]}),
        1,
    )
    .await;
    memory::Entity::update_many()
        .col_expr(
            memory::Column::DueAt,
            sea_orm::sea_query::Expr::value(Utc::now().fixed_offset() - Duration::seconds(1)),
        )
        .filter(memory::Column::UserId.eq(app.owner_id))
        .exec(app.db.as_ref())
        .await
        .unwrap();
    let second = next(&app, &app.owner_token, json!({"run_id":run})).await;
    let ticket = second["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 3).await;
    let attempt2 = attempts::Entity::find_by_id(uuid::Uuid::parse_str(ticket).unwrap())
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(attempt.card_id, attempt2.card_id);
    let summary = app
        .api
        .get(&format!("/api/v1/flashcards/runs/{run}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    assert_eq!(summary["data"]["reviewed"], 2);
    assert_eq!(summary["data"]["unique_reviewed"], 1);
    app.cleanup().await;
}

#[tokio::test]
async fn flashcard_command_summaries_are_user_scoped_idempotent_and_finish_with_next() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    save(&app, session, &doc(vec![entry("remember", "nhớ", &[])]), 0).await;
    let created = app
        .api
        .post("/api/v1/flashcards/runs")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({}))
        .await;
    created.assert_status(StatusCode::CREATED);
    let run = created.json::<Value>()["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let next = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"run_id":run}))
        .await;
    next.assert_status_ok();
    let data = next.json::<Value>()["data"].clone();
    assert_eq!(data["run"]["reviewed"], 0);
    assert_eq!(data["run"]["id"], run);
    assert!(data["card"].get("back").is_none());
    let ticket = data["card"]["id"].as_str().unwrap();
    app.api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"run_id":run}))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.api
        .post(&format!("/api/v1/flashcards/{ticket}/reveal"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status_ok();
    for _ in 0..2 {
        let graded = app
            .api
            .post(&format!("/api/v1/flashcards/{ticket}/grade"))
            .add_header("authorization", auth(&app.owner_token))
            .json(&json!({"rating":3}))
            .await;
        graded.assert_status_ok();
        let summary = graded.json::<Value>()["data"]["run"].clone();
        assert_eq!(summary["reviewed"], 1);
        assert_eq!(summary["unique_reviewed"], 1);
        assert_eq!(summary["ratings"], json!([0, 0, 1, 0]));
    }
    let next = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"run_id":run}))
        .await;
    next.assert_status_ok();
    let data = next.json::<Value>()["data"].clone();
    assert!(data["card"].is_null());
    assert_eq!(data["run"]["completed"], true);
    assert_eq!(data["run"]["reviewed"], 1);
    assert!(data["run"]["finished_at"].is_string());
    app.cleanup().await;
}

#[tokio::test]
async fn two_senses_in_one_entry_share_one_schedule_per_direction_without_repeating_after_good() {
    use server::entities::notes;
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    let meanings = [
        "nhớ, ghi nhớ thông tin hoặc một người",
        "nhớ phải làm điều gì, không quên thực hiện",
    ];
    let mut word = typed_entry("remember", meanings[0], "verb");
    let other = typed_entry("remember", meanings[1], "verb");
    word["content"]
        .as_array_mut()
        .unwrap()
        .push(other["content"][0].clone());
    let content = doc(vec![word]);
    save(&app, session, &content, 0).await;
    let before = notes::Entity::find_by_id(session)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let filter = json!({"session_id":session});
    let first = next(&app, &app.owner_token, filter.clone()).await;
    assert_eq!(first["overview"]["word_count"], 1);
    assert_eq!(first["overview"]["reviewable"], 1);
    assert_eq!(first["overview"]["new"], 1);
    assert_eq!(first["card"]["direction"], "vi_en");
    for meaning in meanings {
        assert!(first["card"]["front"].as_str().unwrap().contains(meaning));
    }
    let ticket = first["card"]["id"].as_str().unwrap();
    let back = reveal(&app, &app.owner_token, ticket).await;
    for meaning in meanings {
        assert!(back["back"]["meaning"].as_str().unwrap().contains(meaning));
    }
    grade(&app, ticket, 3).await;
    let following = next(&app, &app.owner_token, filter.clone()).await;
    assert!(following["card"].is_null());
    assert_eq!(following["overview"]["new"], 0);
    assert_eq!(following["overview"]["due"], 0);
    assert_eq!(
        memory::Entity::find()
            .filter(memory::Column::UserId.eq(app.owner_id))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        1
    );

    // The opposite direction is an intentional separate schedule, not a second sense.
    let both = settings(&app, filter.clone(), "both", 0.9).await;
    assert_eq!(both["new"], 1);
    assert_eq!(both["reviewable"], 1);
    let reverse = next(&app, &app.owner_token, filter.clone()).await;
    assert_eq!(reverse["card"]["direction"], "en_vi");
    assert_eq!(reverse["card"]["front"], "remember");
    let ticket = reverse["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 3).await;
    assert!(next(&app, &app.owner_token, filter).await["card"].is_null());
    assert_eq!(
        memory::Entity::find()
            .filter(memory::Column::UserId.eq(app.owner_id))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        notes::Entity::find_by_id(session)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap(),
        before
    );
    app.cleanup().await;
}

#[tokio::test]
async fn memory_progress_estimates_recall_without_duplicates_or_schedule_writes() {
    let app = LearningApp::spawn().await;
    let first = lesson(&app, &app.owner_token).await;
    let second = lesson(&app, &app.owner_token).await;
    save(
        &app,
        first,
        &doc(vec![
            typed_entry("remember", "nhớ", "verb"),
            typed_entry("learn", "học", "verb"),
        ]),
        0,
    )
    .await;
    save(
        &app,
        second,
        &doc(vec![
            typed_entry("remember", "ghi nhớ", "verb"),
            typed_entry("remember", "ký ức", "noun"),
        ]),
        0,
    )
    .await;
    async fn overview(app: &LearningApp, token: &str, query: &str) -> Value {
        let response = app
            .api
            .get(&format!("/api/v1/flashcards{query}"))
            .add_header("authorization", auth(token))
            .await;
        response.assert_status_ok();
        response.json::<Value>()["data"]["memory_progress"].clone()
    }
    let empty = overview(&app, &app.other_token, "").await;
    assert_eq!(empty["total"], 0);
    let before = overview(&app, &app.owner_token, "").await;
    assert_eq!(before["total"], 2);
    assert_eq!(before["unseen"], 2);
    assert_eq!(before["remembered"], 0.0);
    let card = next(
        &app,
        &app.owner_token,
        json!({"session_id":first,"q":"remember"}),
    )
    .await;
    let ticket = card["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    assert_eq!(overview(&app, &app.owner_token, "").await["unseen"], 2);
    grade(&app, ticket, 3).await;
    let immediate = overview(&app, &app.owner_token, "").await;
    assert_eq!(immediate["unseen"], 1);
    assert!(immediate["remembered"].as_f64().unwrap() > 0.99);
    assert!(immediate["remembered"].as_f64().unwrap() <= 1.0);
    // Use a known memory age/stability. Due time itself is not proof of forgetting.
    let original = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut aged: memory::ActiveModel = original.into();
    aged.stability = Set(10.0);
    aged.reviewed_at = Set(Utc::now().fixed_offset() - Duration::days(10));
    aged.due_at = Set(Utc::now().fixed_offset() - Duration::days(2));
    let aged = aged.update(app.db.as_ref()).await.unwrap();
    let now = overview(&app, &app.owner_token, "").await;
    let remembered = now["remembered"].as_f64().unwrap();
    assert!((remembered - 0.9).abs() < 0.001);
    assert!((now["forgotten"].as_f64().unwrap() - 0.1).abs() < 0.001);
    assert_eq!(now["unseen"], 1);
    assert_eq!(now["total"], 2);
    // Duplicates and another POS in a different lesson never multiply a word.
    let scoped = overview(&app, &app.owner_token, &format!("?session_id={second}")).await;
    assert_eq!(scoped["total"], 1);
    assert_eq!(scoped["unseen"], 0);
    assert!((scoped["remembered"].as_f64().unwrap() - 0.9).abs() < 0.001);
    assert_eq!(
        overview(&app, &app.owner_token, "?q=learn").await["unseen"],
        1
    );
    let other = server::entities::users::Entity::find()
        .filter(server::entities::users::Column::Email.eq("other@example.test"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    server::entities::note_collaborators::ActiveModel {
        session_id: Set(second),
        user_id: Set(other.id),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let shared = overview(&app, &app.other_token, "").await;
    assert_eq!(shared["total"], 1);
    assert_eq!(shared["remembered"], 0.0);
    assert_eq!(shared["unseen"], 1);
    let persisted = memory::Entity::find()
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted, aged);
    settings(&app, json!({}), "en_vi", 0.9).await;
    assert_eq!(overview(&app, &app.owner_token, "").await["unseen"], 2);
    settings(&app, json!({}), "both", 0.9).await;
    let both = overview(&app, &app.owner_token, "").await;
    assert_eq!(both["unseen"], 1);
    assert!((both["remembered"].as_f64().unwrap() - 0.9).abs() < 0.001);
    settings(&app, json!({"session_id":first}), "both", 0.9).await;
    let reverse = next(
        &app,
        &app.owner_token,
        json!({"new_only":true,"q":"remember","session_id":first}),
    )
    .await;
    let ticket = reverse["card"]["id"].as_str().unwrap();
    reveal(&app, &app.owner_token, ticket).await;
    grade(&app, ticket, 4).await;
    let both = overview(&app, &app.owner_token, "").await;
    assert_eq!(both["total"], 2);
    // Weakest studied schedule wins instead of adding two directions together.
    assert!((both["remembered"].as_f64().unwrap() - 0.9).abs() < 0.001);
    server::entities::flashcard_models::ActiveModel {
        user_id: Set(app.owner_id),
        parameters: Set(json!(fsrs::DEFAULT_PARAMETERS)),
        version: Set(1),
        trained_reviews: Set(2),
        training_samples: Set(2),
        baseline_loss: Set(0.5),
        fitted_loss: Set(0.4),
        optimized_at: Set(Some(Utc::now().fixed_offset())),
        checked_at: Set(Utc::now().fixed_offset()),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let schedules_before = memory::Entity::find().all(app.db.as_ref()).await.unwrap();
    let refreshed = overview(&app, &app.owner_token, "").await;
    let replay = fsrs::FSRS::new(&fsrs::DEFAULT_PARAMETERS)
        .unwrap()
        .memory_state(
            fsrs::FSRSItem {
                reviews: vec![fsrs::FSRSReview {
                    rating: 3,
                    delta_t: 0,
                }],
            },
            None,
        )
        .unwrap();
    let expected = fsrs::current_retrievability(replay, 10.0, fsrs::DEFAULT_PARAMETERS[20]) as f64;
    assert!((refreshed["remembered"].as_f64().unwrap() - expected).abs() < 0.001);
    assert_eq!(
        memory::Entity::find().all(app.db.as_ref()).await.unwrap(),
        schedules_before
    );
    app.cleanup().await;
}
