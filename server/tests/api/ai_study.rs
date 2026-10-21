use super::{learning_helpers::LearningApp, media::lesson};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::{ai_imports, exercises};
fn auth(app: &LearningApp) -> String {
    format!("Bearer {}", app.owner_token)
}
fn payload(key: uuid::Uuid) -> Value {
    json!({"key":key,"content":{"type":"doc","content":[{"type":"horizontalRule"},{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Học cùng AI"}]},{"type":"paragraph","content":[{"type":"text","text":"Jobs"}]},{"type":"exercise","attrs":{"exerciseId":1}}]},"exercises":[{"title":"Jobs","questions":[{"type":"fill","prompt":"Work ___ a teacher","options":[],"answers":["as"],"explanation":"Work as + job","caseSensitive":false}]}]})
}
async fn prepare(app: &LearningApp, s: i64, body: &Value) -> Value {
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{s}/ai-study/imports"))
        .add_header("authorization", auth(app))
        .json(body)
        .await;
    r.assert_status_ok();
    r.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn ai_prepare_retries_return_same_ids_and_reject_changed_key_payload_or_other_users() {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let key = uuid::Uuid::new_v4();
    let body = payload(key);
    let a = prepare(&app, s, &body).await;
    let b = prepare(&app, s, &body).await;
    assert_eq!(a["content"], b["content"]);
    assert_eq!(a["content"][1]["attrs"]["aiImportId"], key.to_string());
    assert!(!a.to_string().contains("answers"));
    assert_eq!(
        exercises::Entity::find()
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        1
    );
    let mut changed = body.clone();
    changed["exercises"][0]["title"] = json!("New");
    app.api
        .post(&format!("/api/v1/sessions/{s}/ai-study/imports"))
        .add_header("authorization", auth(&app))
        .json(&changed)
        .await
        .assert_status(StatusCode::CONFLICT);
    app.api
        .post(&format!("/api/v1/sessions/{s}/ai-study/imports"))
        .json(&body)
        .await
        .assert_status_unauthorized();
    app.api
        .post(&format!("/api/v1/sessions/{s}/ai-study/imports"))
        .add_header("authorization", format!("Bearer {}", app.other_token))
        .json(&body)
        .await
        .assert_status_not_found();
    let other = lesson(&app).await;
    app.api
        .post(&format!("/api/v1/sessions/{other}/ai-study/imports"))
        .add_header("authorization", auth(&app))
        .json(&body)
        .await
        .assert_status_not_found();
    for mut invalid in [payload(uuid::Uuid::new_v4()), payload(uuid::Uuid::new_v4())] {
        invalid["content"]["content"][3]["attrs"]["exerciseId"] = json!(999);
        app.api
            .post(&format!("/api/v1/sessions/{s}/ai-study/imports"))
            .add_header("authorization", auth(&app))
            .json(&invalid)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        exercises::Entity::find()
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        1
    );
    app.cleanup().await;
}
#[tokio::test]
async fn ai_note_save_attaches_in_same_transaction_conflicts_do_not_attach_and_cleanup_retains_undo(
) {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let used = uuid::Uuid::new_v4();
    let pending = uuid::Uuid::new_v4();
    let a = prepare(&app, s, &payload(used)).await;
    let b = prepare(&app, s, &payload(pending)).await;
    let note_path = format!("/api/v1/sessions/{s}/note");
    let save = json!({"content":{"type":"doc","content":a["content"]},"schema_version":4,"base_revision":0});
    app.api
        .put(&note_path)
        .add_header("authorization", auth(&app))
        .json(&save)
        .await
        .assert_status_ok();
    let failed = json!({"content":{"type":"doc","content":b["content"]},"schema_version":4,"base_revision":0});
    app.api
        .put(&note_path)
        .add_header("authorization", auth(&app))
        .json(&failed)
        .await
        .assert_status(StatusCode::CONFLICT);
    assert!(
        ai_imports::Entity::find_by_id(used)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .attached
    );
    assert!(
        !ai_imports::Entity::find_by_id(pending)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .attached
    );
    // Removing the section must retain its resources for undo.
    app.api.put(&note_path).add_header("authorization",auth(&app)).json(&json!({"content":{"type":"doc","content":[{"type":"paragraph"}]},"schema_version":4,"base_revision":1})).await.assert_status_ok();
    for key in [used, pending] {
        let model = ai_imports::Entity::find_by_id(key)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        let mut m: ai_imports::ActiveModel = model.into();
        m.created_at = Set(chrono::Utc::now().fixed_offset() - chrono::Duration::days(2));
        m.update(app.db.as_ref()).await.unwrap();
    }
    let removed = server::clean_pending_ai_imports(
        app.db.as_ref(),
        chrono::Utc::now().fixed_offset() - chrono::Duration::hours(24),
    )
    .await
    .unwrap();
    assert_eq!(removed, 1);
    assert!(ai_imports::Entity::find_by_id(used)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_some());
    assert!(ai_imports::Entity::find_by_id(pending)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_none());
    let id = a["content"][3]["attrs"]["exerciseId"].as_i64().unwrap();
    assert!(exercises::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        exercises::Entity::find()
            .filter(exercises::Column::SessionId.eq(s))
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        1
    );
    app.api
        .put(&note_path)
        .add_header("authorization", auth(&app))
        .json(&json!({"content":save["content"],"schema_version":4,"base_revision":2}))
        .await
        .assert_status_ok();
    app.cleanup().await;
}
#[tokio::test]
async fn ai_realtime_snapshot_attaches_import_and_invalid_cross_lesson_marker_is_rejected() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let s = lesson(&app).await;
    let other = lesson(&app).await;
    let key = uuid::Uuid::new_v4();
    let a = prepare(&app, s, &payload(key)).await;
    let body = json!({"content":{"type":"doc","content":a["content"]},"schema_version":4,"state":"AAA=","base_revision":0});
    let secret = "test-collaboration-secret-32-characters";
    app.api
        .put(&format!("/api/v1/internal/notes/{other}"))
        .add_header("x-collaboration-secret", secret)
        .json(&body)
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.api
        .put(&format!("/api/v1/internal/notes/{s}"))
        .add_header("x-collaboration-secret", secret)
        .json(&body)
        .await
        .assert_status_ok();
    assert!(
        ai_imports::Entity::find_by_id(key)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .attached
    );
    app.cleanup().await;
}
