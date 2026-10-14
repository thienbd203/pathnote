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
    let path = format!("/api/v1/sessions/{id}/collaboration/authorize");
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
    let payload = json!({"content":doc(),"state":"AAA=","base_revision":0});
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
        .json(&json!({"content":doc(),"state":"AQI=","base_revision":0}))
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
            .json(&json!({"content":content,"state":state,"base_revision":1}))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", format!("Bearer {}", app.owner_token))
        .json(&json!({"content":doc(),"schema_version":3,"base_revision":1}))
        .await
        .assert_status(StatusCode::BAD_REQUEST);
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
