use super::{
    learning_helpers::LearningApp,
    media::{form, lesson},
};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::{note_collaborators, users};
fn bearer(t: &str) -> String {
    format!("Bearer {t}")
}
async fn upload(app: &LearningApp, session: i64, name: &str, bytes: &[u8]) -> i64 {
    let result = app
        .api
        .post(&format!("/api/v1/sessions/{session}/files"))
        .add_header("authorization", bearer(&app.owner_token))
        .multipart(form("file", name, bytes.to_vec()))
        .await;
    result.assert_status(StatusCode::CREATED);
    result.json::<Value>()["data"]["id"].as_i64().unwrap()
}
async fn ticket(app: &LearningApp, session: i64, media: i64, token: &str) -> String {
    let result = app
        .api
        .post(&format!(
            "/api/v1/sessions/{session}/recordings/{media}/playback"
        ))
        .add_header("authorization", bearer(token))
        .await;
    result.assert_status_ok();
    result.json::<Value>()["data"]["ticket"]
        .as_str()
        .unwrap()
        .into()
}
#[tokio::test]
async fn recordings_stream_ranges_and_restore_personal_progress_without_note_writes() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    let bytes = include_bytes!("../fixtures/lesson.mp3");
    let id = upload(&app, session, "IPA.mp3", bytes).await;
    let video = upload(
        &app,
        session,
        "Bai hoc.mp4",
        include_bytes!("../fixtures/lesson.mp4"),
    )
    .await;
    let list_path = format!("/api/v1/sessions/{session}/recordings");
    app.api.get(&list_path).await.assert_status_unauthorized();
    app.api
        .get(&list_path)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    let list = app
        .api
        .get(&list_path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    list.assert_status_ok();
    let rows = list.json::<Value>()["data"].clone();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["position_seconds"], 0.0);
    assert_eq!(rows[1]["mime_type"], "video/mp4");
    assert!(rows[0].get("storage_key").is_none());
    let token = ticket(&app, session, id, &app.owner_token).await;
    let stream = format!("{list_path}/{id}/stream?ticket={token}");
    app.api
        .get(&format!("{list_path}/{id}/stream"))
        .await
        .assert_status_unauthorized();
    let result = app
        .api
        .get(&stream)
        .add_header("range", "bytes=10-29")
        .await;
    result.assert_status(StatusCode::PARTIAL_CONTENT);
    assert_eq!(result.as_bytes(), &bytes[10..30]);
    assert_eq!(result.header("accept-ranges"), "bytes");
    assert_eq!(
        result.header("content-range"),
        format!("bytes 10-29/{}", bytes.len())
    );
    assert_eq!(result.header("content-type"), "audio/mpeg");
    assert_eq!(result.header("cache-control"), "private, no-store");
    app.api
        .get(&stream)
        .add_header("range", "bytes=999999999-")
        .await
        .assert_status(StatusCode::RANGE_NOT_SATISFIABLE);
    app.api
        .get(&format!("{list_path}/{video}/stream?ticket={token}"))
        .await
        .assert_status_unauthorized();
    app.api
        .get(&format!(
            "{list_path}/{id}/stream?ticket={}",
            app.owner_token
        ))
        .await
        .assert_status_unauthorized();
    app.api
        .get(&list_path)
        .add_header("authorization", bearer(&token))
        .await
        .assert_status_unauthorized();
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.typ = Some("playback+jwt".into());
    header.kid = Some("test-access-key".into());
    let expired = jsonwebtoken::encode(&header, &json!({
        "user":app.owner_id, "sub":app.owner_id.to_string(), "session":session, "media":id,
        "exp":jsonwebtoken::get_current_timestamp()-60, "iss":"https://api.example.test", "aud":"pathnote-api", "token_use":"playback"
    }), &jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!("../fixtures/test-only-rsa.pem")).unwrap()).unwrap();
    app.api
        .get(&format!("{list_path}/{id}/stream?ticket={expired}"))
        .await
        .assert_status_unauthorized();
    app.api
        .get(&format!("{stream}tampered"))
        .await
        .assert_status_unauthorized();
    let progress = format!("{list_path}/{id}/progress");
    for value in [json!(-1), json!(604801), json!("wrong")] {
        app.api
            .put(&progress)
            .add_header("authorization", bearer(&app.owner_token))
            .json(&json!({"position_seconds":value}))
            .await
            .assert_status_unprocessable_entity();
    }
    app.api
        .put(&progress)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"position_seconds":4.25}))
        .await
        .assert_status_ok();
    let list = app
        .api
        .get(&list_path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(list.json::<Value>()["data"][0]["position_seconds"], 4.25);
    // A new request (like F5 / another device) reads persisted progress. No note is created.
    assert!(server::entities::notes::Entity::find_by_id(session)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_none());
    let other = users::Entity::find()
        .filter(users::Column::Email.eq("other@example.test"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    note_collaborators::ActiveModel {
        session_id: Set(session),
        user_id: Set(other.id),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    let list = app
        .api
        .get(&list_path)
        .add_header("authorization", bearer(&app.other_token))
        .await;
    list.assert_status_ok();
    assert_eq!(list.json::<Value>()["data"][0]["position_seconds"], 0.0);
    app.api
        .put(&progress)
        .add_header("authorization", bearer(&app.other_token))
        .json(&json!({"position_seconds":8.0}))
        .await
        .assert_status_ok();
    let list = app
        .api
        .get(&list_path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(list.json::<Value>()["data"][0]["position_seconds"], 4.25);
    let other_ticket = ticket(&app, session, id, &app.other_token).await;
    note_collaborators::Entity::delete_by_id((session, other.id))
        .exec(app.db.as_ref())
        .await
        .unwrap();
    app.api
        .get(&format!("{list_path}/{id}/stream?ticket={other_ticket}"))
        .await
        .assert_status_not_found();
    app.api
        .put(&progress)
        .add_header("authorization", bearer(&app.other_token))
        .json(&json!({"position_seconds":9.0}))
        .await
        .assert_status_not_found();
    let another = lesson(&app).await;
    app.api
        .post(&format!(
            "/api/v1/sessions/{another}/recordings/{id}/playback"
        ))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
#[tokio::test]
async fn recording_upload_rejects_disguised_files_and_retains_document_limit() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    for (name, bytes) in [
        ("fake.mp3", b"<html>not audio</html>".to_vec()),
        (
            "fake.mp4",
            include_bytes!("../fixtures/lesson.mp3").to_vec(),
        ),
        ("big.txt", vec![b'a'; 10 * 1024 * 1024 + 1]),
    ] {
        app.api
            .post(&format!("/api/v1/sessions/{session}/files"))
            .add_header("authorization", bearer(&app.owner_token))
            .multipart(form("file", name, bytes))
            .await
            .assert_status_unprocessable_entity();
    }
    app.cleanup().await;
}

#[tokio::test]
async fn audio_block_saves_as_a_private_file_reference_and_progress_does_not_edit_the_note() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    let id = upload(
        &app,
        session,
        "Teacher.mp3",
        include_bytes!("../fixtures/lesson.mp3"),
    )
    .await;
    let note_path = format!("/api/v1/sessions/{session}/note");
    let content = json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"Before audio"}]},
        {"type":"file","attrs":{"mediaId":id}},
        {"type":"paragraph","content":[{"type":"text","text":"After audio"}]}
    ]});
    let saved = app
        .api
        .put(&note_path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"content":content,"schema_version":4,"base_revision":0}))
        .await;
    saved.assert_status_ok();
    let before = saved.json::<Value>()["data"].clone();
    assert_eq!(
        before["content"]["content"][1],
        json!({"type":"file","attrs":{"mediaId":id}})
    );
    let progress = format!("/api/v1/sessions/{session}/recordings/{id}/progress");
    app.api.get(&progress).await.assert_status_unauthorized();
    app.api
        .get(&progress)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    let read = app
        .api
        .get(&progress)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    read.assert_status_ok();
    assert_eq!(read.json::<Value>()["data"]["position_seconds"], 0.0);
    app.api
        .put(&progress)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"position_seconds":5.5}))
        .await
        .assert_status_ok();
    let read = app
        .api
        .get(&progress)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(read.json::<Value>()["data"]["position_seconds"], 5.5);
    let after = app
        .api
        .get(&note_path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(after.json::<Value>()["data"], before);
    let other_session = lesson(&app).await;
    app.api
        .get(&format!(
            "/api/v1/sessions/{other_session}/recordings/{id}/progress"
        ))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    app.api
        .put(&format!("/api/v1/sessions/{other_session}/note"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"content":content,"schema_version":4,"base_revision":0}))
        .await
        .assert_status_unprocessable_entity();
    app.cleanup().await;
}

#[tokio::test]
async fn renaming_media_keeps_bytes_tickets_progress_and_rejects_foreign_or_invalid_names() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app).await;
    let bytes = include_bytes!("../fixtures/lesson.mp3");
    let id = upload(&app, session, "Original.mp3", bytes).await;
    let path = format!("/api/v1/sessions/{session}/files/{id}");
    let token = ticket(&app, session, id, &app.owner_token).await;
    let original = server::entities::media::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let progress = format!("/api/v1/sessions/{session}/recordings/{id}/progress");
    app.api
        .put(&progress)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"position_seconds":4.5}))
        .await
        .assert_status_ok();
    app.api
        .patch(&path)
        .json(&json!({"name":"New.mp3"}))
        .await
        .assert_status_unauthorized();
    app.api
        .patch(&path)
        .add_header("authorization", bearer(&app.other_token))
        .json(&json!({"name":"New.mp3"}))
        .await
        .assert_status_not_found();
    for name in [
        "",
        ".mp3",
        "  .mp3",
        "New.wav",
        "../escape.mp3",
        "a\\b.mp3",
        "a\nb.mp3",
        "a\"b.mp3",
    ] {
        app.api
            .patch(&path)
            .add_header("authorization", bearer(&app.owner_token))
            .json(&json!({"name":name}))
            .await
            .assert_status_unprocessable_entity();
    }
    app.api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"a".repeat(201)+".mp3"}))
        .await
        .assert_status_unprocessable_entity();
    app.api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"New.mp3","storage_key":"evil"}))
        .await
        .assert_status_unprocessable_entity();
    let result = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"  Luyện nghe IPA.mp3  "}))
        .await;
    result.assert_status_ok();
    assert_eq!(result.json::<Value>()["data"]["name"], "Luyện nghe IPA.mp3");
    let current = server::entities::media::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.storage_key, original.storage_key);
    assert_eq!(current.mime_type, original.mime_type);
    assert_eq!(current.size, original.size);
    assert_eq!(
        app.api
            .get(&progress)
            .add_header("authorization", bearer(&app.owner_token))
            .await
            .json::<Value>()["data"]["position_seconds"],
        4.5
    );
    let stream = app
        .api
        .get(&format!(
            "/api/v1/sessions/{session}/recordings/{id}/stream?ticket={token}"
        ))
        .add_header("range", "bytes=0-19")
        .await;
    stream.assert_status(StatusCode::PARTIAL_CONTENT);
    assert_eq!(stream.as_bytes(), &bytes[..20]);
    let other_session = lesson(&app).await;
    app.api
        .patch(&format!("/api/v1/sessions/{other_session}/files/{id}"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"Other.mp3"}))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
