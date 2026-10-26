use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use fsrs::{FSRSItem, FSRSReview, FSRS};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{json, Value};
use server::entities::{
    flashcard_attempts as attempts, flashcard_memory as memory, flashcard_models as models,
    vocabulary_cards as cards,
};
use sha2::{Digest, Sha256};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn status(app: &LearningApp, token: &str) -> Value {
    let r = app
        .api
        .get("/api/v1/flashcards/optimization")
        .add_header("authorization", auth(token))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn next(app: &LearningApp, q: &str) -> Value {
    let r = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"q":q}))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"]["card"].clone()
}
async fn reveal(app: &LearningApp, id: &str) -> Value {
    let r = app
        .api
        .post(&format!("/api/v1/flashcards/{id}/reveal"))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
async fn grade(app: &LearningApp, id: &str, rating: u8) {
    app.api
        .post(&format!("/api/v1/flashcards/{id}/grade"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"rating":rating}))
        .await
        .assert_status_ok();
}
async fn seed_history(app: &LearningApp) -> Vec<cards::Model> {
    let lesson = lesson(app, &app.owner_token).await;
    save(
        app,
        lesson,
        &doc((0..40)
            .map(|i| entry(&format!("teacher {i:03}"), "giáo viên", &["Jobs"]))
            .collect()),
        0,
    )
    .await;
    let rows = cards::Entity::find()
        .order_by_asc(cards::Column::Word)
        .all(app.db.as_ref())
        .await
        .unwrap();
    let first = Utc::now().fixed_offset() - Duration::days(30);
    for (i, c) in rows.iter().enumerate() {
        let identity = json!([c.word_key, c.locale, c.part_of_speech]);
        let content_key = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([c.word, c.meaning, c.locale, c.part_of_speech]))
                    .unwrap()
            )
        );
        let key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&identity).unwrap())
        );
        let ratings = [
            3,
            if i % 4 == 0 { 3 } else { 1 },
            if i % 3 == 0 { 1 } else { 3 },
        ];
        let days = [0, 1, 4];
        let deltas = [0, 1, 3];
        let mut state = None;
        for n in 0..3 {
            let next = FSRS::default().next_states(state, 0.9, deltas[n]).unwrap();
            state = Some(if ratings[n] == 1 {
                next.again.memory
            } else {
                next.good.memory
            });
            attempts::ActiveModel {
                run_id: Set(None),
                id: Set(uuid::Uuid::new_v4()),
                user_id: Set(app.owner_id),
                card_id: Set(c.id),
                memory_card_id: Set(c.id),
                content_key: Set(content_key.clone()),
                direction: Set("vi_en".into()),
                study_key: Set(key.clone()),
                version: Set(n as i32),
                parameter_version: Set(0),
                parameters: Set(None),
                snapshot: Set(None),
                delta_days: Set(Some(deltas[n] as i32)),
                retention: Set(0.9),
                revealed: Set(true),
                rating: Set(Some(ratings[n])),
                due_at: Set(Some(first + Duration::days(days[n] + 1))),
                reviewed_at: Set(Some(first + Duration::days(days[n]))),
                created_at: Set(first + Duration::days(days[n])),
            }
            .insert(app.db.as_ref())
            .await
            .unwrap();
        }
        let state = state.unwrap();
        memory::ActiveModel {
            user_id: Set(app.owner_id),
            card_id: Set(c.id),
            direction: Set("vi_en".into()),
            identity: Set(identity),
            study_key: Set(key),
            parameter_version: Set(0),
            stability: Set(state.stability),
            difficulty: Set(state.difficulty),
            step: Set(0),
            reviews: Set(3),
            lapses: Set(ratings.iter().filter(|r| **r == 1).count() as i32),
            due_at: Set(Utc::now().fixed_offset() - Duration::hours(1)),
            reviewed_at: Set(first + Duration::days(4)),
        }
        .insert(app.db.as_ref())
        .await
        .unwrap();
    }
    rows
}
#[tokio::test]
async fn automatic_optimizer_fits_real_library_keeps_due_dates_and_snapshots_and_replays_memory() {
    let app = LearningApp::spawn().await;
    let cards = seed_history(&app).await;
    let old = next(&app, "teacher 000").await;
    let old_id = old["id"].as_str().unwrap();
    let before = reveal(&app, old_id).await;
    let info = status(&app, &app.owner_token).await;
    assert_eq!(info["automatic"], true);
    assert_eq!(info["samples"], 80);
    assert_eq!(info["reviews"], 120);
    assert_eq!(info["state"], "pending");
    let due_before = memory::Entity::find()
        .order_by_asc(memory::Column::CardId)
        .all(app.db.as_ref())
        .await
        .unwrap();
    assert_eq!(
        server::optimize_fsrs_once(app.db.as_ref()).await.unwrap(),
        1
    );
    let model = models::Entity::find_by_id(app.owner_id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(model.version, 1);
    assert_eq!(model.trained_reviews, 120);
    assert_eq!(model.training_samples, 80);
    assert!(model.fitted_loss < model.baseline_loss);
    assert!(model.optimized_at.is_some());
    let due_after = memory::Entity::find()
        .order_by_asc(memory::Column::CardId)
        .all(app.db.as_ref())
        .await
        .unwrap();
    assert_eq!(due_before, due_after);
    assert_eq!(
        reveal(&app, old_id).await["options"][3]["seconds"],
        before["options"][3]["seconds"]
    );
    grade(&app, old_id, 4).await;
    assert_eq!(status(&app, &app.other_token).await["personalized"], false);
    let info = status(&app, &app.owner_token).await;
    assert_eq!(info["state"], "personal");
    assert_eq!(info["version"], 1);
    // A second pass (including after restart) uses persisted metadata and does no fitting again.
    assert_eq!(
        server::optimize_fsrs_once(app.db.as_ref()).await.unwrap(),
        0
    );
    let card = next(&app, "teacher 001").await;
    let id = card["id"].as_str().unwrap();
    let back = reveal(&app, id).await;
    let parameters: Vec<f32> = serde_json::from_value(model.parameters.clone()).unwrap();
    let fsrs = FSRS::new(&parameters).unwrap();
    let item = FSRSItem {
        reviews: vec![
            FSRSReview {
                rating: 3,
                delta_t: 0,
            },
            FSRSReview {
                rating: 1,
                delta_t: 1,
            },
            FSRSReview {
                rating: 3,
                delta_t: 3,
            },
        ],
    };
    let state = fsrs.memory_state(item, None).unwrap();
    let row = memory::Entity::find()
        .filter(memory::Column::UserId.eq(app.owner_id))
        .filter(memory::Column::CardId.eq(cards[1].id))
        .filter(memory::Column::Direction.eq("vi_en"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let elapsed = (Utc::now().fixed_offset() - row.reviewed_at).num_days() as u32;
    let expected = fsrs.next_states(Some(state), 0.9, elapsed).unwrap().good;
    assert_eq!(
        back["options"][2]["stability"].as_f64().unwrap() as f32,
        expected.memory.stability
    );
    grade(&app, id, 3).await;
    let row = memory::Entity::find()
        .filter(memory::Column::UserId.eq(app.owner_id))
        .filter(memory::Column::CardId.eq(cards[1].id))
        .filter(memory::Column::Direction.eq("vi_en"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.parameter_version, 1);
    assert_eq!(row.stability, expected.memory.stability);
    let last = attempts::Entity::find()
        .filter(attempts::Column::Id.eq(uuid::Uuid::parse_str(id).unwrap()))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(last.delta_days, Some(elapsed as i32));
    assert_eq!(last.parameter_version, 1);
    app.cleanup().await;
}
#[tokio::test]
async fn not_enough_history_stays_default_and_no_manual_optimize_endpoint_exists() {
    let app = LearningApp::spawn().await;
    let lesson = lesson(&app, &app.owner_token).await;
    save(
        &app,
        lesson,
        &doc(vec![entry("teacher", "giáo viên", &[])]),
        0,
    )
    .await;
    let card = next(&app, "teacher").await;
    let id = card["id"].as_str().unwrap();
    reveal(&app, id).await;
    grade(&app, id, 3).await;
    let info = status(&app, &app.owner_token).await;
    assert_eq!(info["state"], "collecting");
    assert_eq!(info["reviews"], 1);
    assert_eq!(info["samples"], 0);
    assert_eq!(
        server::optimize_fsrs_once(app.db.as_ref()).await.unwrap(),
        0
    );
    assert!(models::Entity::find_by_id(app.owner_id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_none());
    app.api
        .post("/api/v1/flashcards/optimization")
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::METHOD_NOT_ALLOWED);
    app.api
        .get("/api/v1/flashcards/optimization")
        .await
        .assert_status_unauthorized();
    app.cleanup().await;
}

#[tokio::test]
async fn skipped_optimizer_checks_are_durable_and_do_not_reload_history_on_next_tick() {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::{Arc, Mutex};
    for broken_chain in [true, false] {
        let app = LearningApp::spawn().await;
        seed_history(&app).await;
        if broken_chain {
            // Candidate has enough logged reviews but not one complete FSRS chain.
            app.db
                .execute_unprepared("UPDATE flashcard_attempts SET version=1")
                .await
                .unwrap();
        } else {
            models::ActiveModel {
                user_id: Set(app.owner_id),
                parameters: Set(json!(fsrs::DEFAULT_PARAMETERS)),
                version: Set(0),
                trained_reviews: Set(120),
                training_samples: Set(79),
                baseline_loss: Set(0.5),
                fitted_loss: Set(0.5),
                optimized_at: Set(None),
                evaluated_at: Set(Some(Utc::now().fixed_offset() - Duration::days(1))),
                checked_at: Set(Utc::now().fixed_offset() - Duration::hours(2)),
            }
            .insert(app.db.as_ref())
            .await
            .unwrap();
        }
        let queries = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = queries.clone();
        let mut db = app.db.as_ref().clone();
        db.set_metric_callback(move |info| {
            captured.lock().unwrap().push(info.statement.sql.clone())
        });
        assert_eq!(server::optimize_fsrs_once(&db).await.unwrap(), 0);
        let first = queries
            .lock()
            .unwrap()
            .iter()
            .filter(|sql| sql.contains("flashcard_attempts") && sql.contains("ORDER BY"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(first.len(), 1);
        assert!(!first[0].contains("snapshot"));
        assert!(!first[0].contains("parameters"));
        let checked = models::Entity::find_by_id(app.owner_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(checked.checked_at > Utc::now().fixed_offset() - Duration::minutes(1));
        queries.lock().unwrap().clear();
        assert_eq!(server::optimize_fsrs_once(&db).await.unwrap(), 0);
        assert!(!queries
            .lock()
            .unwrap()
            .iter()
            .any(|sql| sql.contains("flashcard_attempts") && sql.contains("ORDER BY")));
        if !broken_chain {
            // Hourly checks must not indefinitely postpone the independent 30-day evaluation.
            db.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE flashcard_models SET evaluated_at=$1, checked_at=$2 WHERE user_id=$3",
                [
                    (Utc::now().fixed_offset() - Duration::days(31)).into(),
                    (Utc::now().fixed_offset() - Duration::hours(2)).into(),
                    app.owner_id.into(),
                ],
            ))
            .await
            .unwrap();
            server::optimize_fsrs_once(&db).await.unwrap();
            let fitted = models::Entity::find_by_id(app.owner_id)
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            assert!(
                fitted.evaluated_at.unwrap() > Utc::now().fixed_offset() - Duration::minutes(1)
            );
        }
        app.cleanup().await;
    }
}
