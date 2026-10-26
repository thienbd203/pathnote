use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use server::entities::notes;
const SECRET: &str = "test-collaboration-secret-32-characters";
async fn lesson(app: &LearningApp) -> i64 {
    let auth = format!("Bearer {}", app.owner_token);
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", &auth)
        .json(&json!({"name":"Realtime school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let place = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app.api.post(&format!("/api/v1/places/{place}/sessions")).add_header("authorization", &auth).json(&json!({"title":"Realtime","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"Asia/Saigon"})).await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
fn doc() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Original"}]}]})
}
#[tokio::test]
async fn collaboration_requires_bridge_secret_and_note_permission_and_rechecks_revocation() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let id = lesson(&app).await;
    let auth = format!("Bearer {}", app.owner_token);
    let guest = format!("Bearer {}", app.other_token);
    let path = format!("/api/v1/sessions/{id}/collaboration/authorize?schema_version=4");
    app.api
        .post(&path)
        .add_header("authorization", &auth)
        .await
        .assert_status_forbidden();
    app.api
        .post(&path)
        .add_header("x-collaboration-secret", "wrong")
        .add_header("authorization", &auth)
        .await
        .assert_status_forbidden();
    app.api
        .post(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await
        .assert_status_unauthorized();
    app.api
        .post(&path)
        .add_header("x-collaboration-secret", SECRET)
        .add_header("authorization", &guest)
        .await
        .assert_status_not_found();
    app.api
        .post(&format!("/api/v1/sessions/{id}/collaborators"))
        .add_header("authorization", &auth)
        .json(&json!({"email":"other@example.test"}))
        .await
        .assert_status(StatusCode::CREATED);
    let r = app
        .api
        .post(&path)
        .add_header("x-collaboration-secret", SECRET)
        .add_header("authorization", &guest)
        .await;
    r.assert_status_ok();
    let user = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    assert_eq!(r.json::<Value>()["data"].as_object().unwrap().len(), 2);
    app.api
        .delete(&format!("/api/v1/sessions/{id}/collaborators/{user}"))
        .add_header("authorization", &auth)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .post(&path)
        .add_header("x-collaboration-secret", SECRET)
        .add_header("authorization", &guest)
        .await
        .assert_status_not_found();
    let config = app
        .api
        .get(&format!("/api/v1/sessions/{id}/collaboration"))
        .add_header("authorization", &auth)
        .await;
    config.assert_status_ok();
    assert_eq!(config.json::<Value>()["data"]["enabled"], true);
    assert!(!config.text().contains(SECRET));
    app.cleanup().await;
}
#[tokio::test]
async fn collaboration_snapshots_are_private_atomic_validated_and_idempotent() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/internal/notes/{id}");
    app.api.get(&path).await.assert_status_forbidden();
    let r = app
        .api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await;
    r.assert_status_ok();
    assert!(r.json::<Value>()["data"]["state"].is_null());
    // Binary interpretation belongs to Hocuspocus; the API checks size/base64 and validates the JSON.
    let payload = json!({"content":doc(),"schema_version":4,"state":"AAA=","base_revision":0});
    let r = app
        .api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&payload)
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["revision"], 1);
    let repeat = app
        .api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&payload)
        .await;
    repeat.assert_status_ok();
    assert_eq!(repeat.json::<Value>()["data"]["revision"], 1);
    let r = app
        .api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&json!({"content":doc(),"schema_version":4,"state":"AQI=","base_revision":0}))
        .await;
    r.assert_status(StatusCode::CONFLICT);
    let r = app
        .api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await;
    assert_eq!(r.json::<Value>()["data"]["state"], "AAA=");
    assert_eq!(r.json::<Value>()["data"]["content"], doc());
    for (content, state) in [
        (doc(), "?"),
        (
            json!({"type":"doc","content":[{"type":"image","attrs":{"mediaId":999}}]}),
            "AQI=",
        ),
        (json!({"type":"unknown"}), "AQI="),
    ] {
        app.api
            .put(&path)
            .add_header("x-collaboration-secret", SECRET)
            .json(&json!({"content":content,"schema_version":4,"state":state,"base_revision":1}))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", format!("Bearer {}", app.owner_token))
        .json(&json!({"content":doc(),"schema_version":4,"base_revision":1}))
        .await
        .assert_status(StatusCode::CONFLICT);
    app.api
        .delete(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", format!("Bearer {}", app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await
        .assert_status_not_found();
    app.api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&payload)
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
#[tokio::test]
async fn disabled_bridge_does_not_expose_internal_notes() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    app.api
        .get(&format!("/api/v1/internal/notes/{id}"))
        .add_header("x-collaboration-secret", SECRET)
        .await
        .assert_status_forbidden();
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{id}/collaboration"))
        .add_header("authorization", format!("Bearer {}", app.owner_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["enabled"], false);
    app.cleanup().await;
}
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Node >=22 and npm ci in the sibling pathnote-collaboration repo"]
async fn hocuspocus_two_clients_save_to_rust_and_reload_from_postgres() {
    let app = LearningApp::spawn_bridge(true, true).await;
    let id = lesson(&app).await;
    notes::ActiveModel {
        session_id: Set(id),
        content: Set(doc()),
        yjs_state: Set(None),
        schema_version: Set(3),
        revision: Set(1),
        saved_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../pathnote-collaboration/tests/rust-bridge.mjs");
    let output =
        tokio::process::Command::new(std::env::var("NODE_BIN").unwrap_or_else(|_| "node".into()))
            .arg(script)
            .env(
                "BRIDGE_API_URL",
                app.api.server_url("/").unwrap().to_string(),
            )
            .env("BRIDGE_OWNER_TOKEN", &app.owner_token)
            .env("BRIDGE_OTHER_TOKEN", &app.other_token)
            .env("BRIDGE_SECRET", SECRET)
            .env("BRIDGE_SESSION", id.to_string())
            .output()
            .await
            .unwrap();
    if !output.status.success() {
        app.cleanup().await;
        panic!(
            "Cross-stack test failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let note = notes::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert!(note.yjs_state.is_some());
    assert!(note.content.to_string().contains("B Original A"));
    app.cleanup().await;
}

#[tokio::test]
async fn schema_guard_rejects_old_bridge_and_old_realtime_clients_before_writing_or_syncing() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/internal/notes/{id}");
    let versioned = json!({"content":doc(),"schema_version":4,"state":"AAA=","base_revision":0});
    app.api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&versioned)
        .await
        .assert_status_ok();
    for input in [
        json!({"content":doc(),"schema_version":3,"state":"AQI=","base_revision":1}),
        json!({"content":doc(),"state":"AQI=","base_revision":1}),
    ] {
        let response = app
            .api
            .put(&path)
            .add_header("x-collaboration-secret", SECRET)
            .json(&input)
            .await;
        response.assert_status(StatusCode::CONFLICT);
        assert_eq!(response.json::<Value>()["code"], "client_outdated");
    }
    let persisted = app
        .api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await
        .json::<Value>();
    assert_eq!(persisted["data"]["state"], "AAA=");
    assert_eq!(persisted["data"]["schema_version"], 4);
    assert_eq!(persisted["data"]["revision"], 1);
    for query in ["", "?schema_version=3"] {
        let response = app
            .api
            .post(&format!(
                "/api/v1/sessions/{id}/collaboration/authorize{query}"
            ))
            .add_header("x-collaboration-secret", SECRET)
            .add_header("authorization", format!("Bearer {}", app.owner_token))
            .await;
        response.assert_status(StatusCode::CONFLICT);
        assert_eq!(response.json::<Value>()["code"], "client_outdated");
    }
    app.cleanup().await;
}

#[tokio::test]
async fn schema_four_transition_accepts_missing_version_only_for_plain_legacy_notes() {
    let app = LearningApp::spawn_with_collaboration(true).await;
    let id = lesson(&app).await;
    // Grace applies to a persisted legacy document, never a newly created note.
    server::entities::notes::ActiveModel {
        session_id: Set(id),
        content: Set(doc()),
        schema_version: Set(3),
        revision: Set(0),
        saved_at: Set(chrono::Utc::now().fixed_offset()),
        yjs_state: Set(None),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let path = format!("/api/v1/internal/notes/{id}");
    app.api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&json!({"content":doc(),"state":"AAA=","base_revision":0}))
        .await
        .assert_status_ok();
    let response = app
        .api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await
        .json::<Value>();
    assert_eq!(response["data"]["schema_version"], 3);
    app.cleanup().await;
}

#[tokio::test]
async fn bridge_reads_note_once_per_request_and_preserves_large_state_on_retry_and_conflict() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let (app, queries) = LearningApp::spawn_with_queries(true).await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/internal/notes/{id}");
    let state = STANDARD.encode(vec![42u8; 2 * 1024 * 1024]);
    let payload = json!({"content":doc(),"schema_version":4,"state":state,"base_revision":0});
    let note_reads = || {
        queries
            .lock()
            .unwrap()
            .iter()
            .filter(|sql| sql.starts_with("SELECT") && sql.contains("FROM \"notes\""))
            .count()
    };
    for _ in 0..2 {
        queries.lock().unwrap().clear();
        let saved = app
            .api
            .put(&path)
            .add_header("x-collaboration-secret", SECRET)
            .json(&payload)
            .await;
        saved.assert_status_ok();
        assert_eq!(saved.json::<Value>()["data"]["revision"], 1);
        assert_eq!(note_reads(), 1);
    }
    queries.lock().unwrap().clear();
    let fetched = app
        .api
        .get(&path)
        .add_header("x-collaboration-secret", SECRET)
        .await;
    fetched.assert_status_ok();
    assert_eq!(note_reads(), 1);
    assert_eq!(fetched.json::<Value>()["data"]["state"], state);
    assert_eq!(fetched.json::<Value>()["data"]["content"], doc());
    let mut stale = payload.clone();
    stale["content"]["content"][0]["content"][0]["text"] = json!("Must not overwrite");
    queries.lock().unwrap().clear();
    let conflict = app
        .api
        .put(&path)
        .add_header("x-collaboration-secret", SECRET)
        .json(&stale)
        .await;
    conflict.assert_status(StatusCode::CONFLICT);
    assert_eq!(note_reads(), 1);
    assert_eq!(conflict.json::<Value>()["data"]["revision"], 1);
    let saved = notes::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.content, doc());
    assert_eq!(saved.revision, 1);
    assert_eq!(STANDARD.encode(saved.yjs_state.unwrap()), state);
    app.cleanup().await;
}
