use super::{
    learning_helpers::LearningApp,
    media::{form, lesson},
};
use axum::http::StatusCode;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set,
    TransactionTrait,
};
use serde_json::{json, Value};
use server::entities::{media, note_collaborators, notes, places, sessions};
use std::{
    fs::{File, FileTimes},
    path::Path,
    time::{Duration, SystemTime},
};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn parent(app: &LearningApp, id: i64) -> i64 {
    sessions::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .place_id
}
fn age(path: &Path) {
    File::open(path)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(48 * 3600)),
        )
        .unwrap();
}
#[tokio::test]
async fn invalid_document_courses_and_sessions_fail_without_waiting_for_place_lock() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let place = parent(&app, id).await;
    let txn = app.db.begin().await.unwrap();
    places::Entity::find_by_id(place)
        .lock_exclusive()
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    let note=tokio::time::timeout(Duration::from_secs(2),app.api.put(&format!("/api/v1/sessions/{id}/note")).add_header("authorization",auth(&app.owner_token)).json(&json!({"content":{"type":"doc","content":[{"type":"invalid"}]},"schema_version":4,"base_revision":0}))).await;
    let course = tokio::time::timeout(
        Duration::from_secs(2),
        app.api
            .post(&format!("/api/v1/places/{place}/courses"))
            .add_header("authorization", auth(&app.owner_token))
            .json(&json!({"name":"Course","starts_on":"2026-10-01","ends_on":"2026-09-01"})),
    )
    .await;
    let session=tokio::time::timeout(Duration::from_secs(2),app.api.post(&format!("/api/v1/places/{place}/sessions")).add_header("authorization",auth(&app.owner_token)).json(&json!({"title":"Invalid timezone","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"No/SuchZone"}))).await;
    txn.rollback().await.unwrap();
    note.expect("document validation waited on the place lock")
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    course
        .expect("course validation waited on the place lock")
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    session
        .expect("session validation waited on the place lock")
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.cleanup().await;
}
#[tokio::test]
async fn note_rechecks_media_references_after_waiting_on_parent_lock() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let other = lesson(&app).await;
    let place = parent(&app, id).await;
    let m = media::ActiveModel {
        session_id: Set(id),
        kind: Set("image".into()),
        name: Set("fixture.png".into()),
        storage_key: Set(uuid::Uuid::new_v4().to_string()),
        mime_type: Set("image/png".into()),
        size: Set(1),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let txn = app.db.begin().await.unwrap();
    places::Entity::find_by_id(place)
        .lock_exclusive()
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    let request=app.api.put(&format!("/api/v1/sessions/{id}/note")).add_header("authorization",auth(&app.owner_token)).json(&json!({"content":{"type":"doc","content":[{"type":"image","attrs":{"mediaId":m.id}}]},"schema_version":4,"base_revision":0}));
    let pending = tokio::spawn(async move { request.await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!pending.is_finished());
    media::Entity::update_many()
        .col_expr(media::Column::SessionId, Expr::value(other))
        .filter(media::Column::Id.eq(m.id))
        .exec(&txn)
        .await
        .unwrap();
    txn.commit().await.unwrap();
    pending
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert!(notes::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_none());
    app.cleanup().await;
}
#[tokio::test]
async fn upload_writes_before_db_lock_cleanup_skips_lease_and_revoke_compensates_file() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let place = parent(&app, id).await;
    let member = app
        .api
        .post(&format!("/api/v1/sessions/{id}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    member.assert_status(StatusCode::CREATED);
    let user = member.json::<Value>()["data"]["id"].as_i64().unwrap();
    let txn = app.db.begin().await.unwrap();
    places::Entity::find_by_id(place)
        .lock_exclusive()
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    let request = app
        .api
        .post(&format!("/api/v1/sessions/{id}/files"))
        .add_header("authorization", auth(&app.other_token))
        .multipart(form("file", "words.txt", b"a teacher".to_vec()));
    let pending = tokio::spawn(async move { request.await });
    let disk = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(mut dir) = tokio::fs::read_dir(&app.media_root).await {
                if let Some(entry) = dir.next_entry().await.unwrap() {
                    break entry.path();
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("upload should reach disk before waiting for the place lock");
    assert!(!pending.is_finished());
    age(&disk);
    let cleaned = server::clean_orphan_media(&app.db, Path::new(&app.media_root), Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(cleaned.removed, 0);
    assert!(disk.exists());
    note_collaborators::Entity::delete_by_id((id, user))
        .exec(&txn)
        .await
        .unwrap();
    txn.commit().await.unwrap();
    pending.await.unwrap().assert_status_not_found();
    assert!(!disk.exists());
    assert!(media::Entity::find()
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    app.cleanup().await;
}
#[tokio::test]
async fn cleanup_removes_only_old_unregistered_files_not_rows_young_files_or_symlinks() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let upload = app
        .api
        .post(&format!("/api/v1/sessions/{id}/files"))
        .add_header("authorization", auth(&app.owner_token))
        .multipart(form("file", "words.txt", b"teacher".to_vec()))
        .await;
    upload.assert_status(StatusCode::CREATED);
    let media_id = upload.json::<Value>()["data"]["id"].as_i64().unwrap();
    let model = media::Entity::find_by_id(media_id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let root = Path::new(&app.media_root);
    let registered = root.join(&model.storage_key);
    age(&registered);
    let orphan = root.join(uuid::Uuid::new_v4().to_string());
    tokio::fs::write(&orphan, b"orphan").await.unwrap();
    age(&orphan);
    let young = root.join(uuid::Uuid::new_v4().to_string());
    tokio::fs::write(&young, b"young").await.unwrap();
    let unrelated = root.join("leave-me.txt");
    tokio::fs::write(&unrelated, b"config").await.unwrap();
    age(&unrelated);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&unrelated, root.join(uuid::Uuid::new_v4().to_string()))
            .unwrap();
    }
    let result = server::clean_orphan_media(&app.db, root, server::MEDIA_CLEANUP_GRACE)
        .await
        .unwrap();
    assert_eq!(result.removed, 1);
    assert!(!orphan.exists());
    assert!(registered.exists());
    assert!(young.exists());
    assert!(unrelated.exists());
    let download = app
        .api
        .get(&format!("/api/v1/sessions/{id}/files/{media_id}/download"))
        .add_header("authorization", auth(&app.owner_token))
        .await;
    download.assert_status_ok();
    assert_eq!(download.as_bytes(), b"teacher".as_slice());
    assert_eq!(
        server::clean_orphan_media(&app.db, root, server::MEDIA_CLEANUP_GRACE)
            .await
            .unwrap()
            .removed,
        0
    );
    app.cleanup().await;
}
