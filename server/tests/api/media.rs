use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use axum_test::multipart::{MultipartForm, Part};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use server::entities::media;
use std::io::{Cursor, Write};
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
pub(super) async fn lesson(app: &LearningApp) -> i64 {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":"Media school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let place = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r=app.api.post(&format!("/api/v1/places/{place}/sessions")).add_header("authorization",bearer(&app.owner_token)).json(&json!({"title":"Lesson","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"Asia/Ho_Chi_Minh"})).await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
pub(super) fn form(field: &str, name: &str, bytes: Vec<u8>) -> MultipartForm {
    MultipartForm::new().add_part(
        field,
        Part::bytes(bytes)
            .file_name(name)
            .mime_type("application/octet-stream"),
    )
}
fn png() -> Vec<u8> {
    let mut b = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2)
        .write_to(&mut b, image::ImageFormat::Png)
        .unwrap();
    b.into_inner()
}
async fn upload(app: &LearningApp, id: i64, kind: &str, name: &str, bytes: Vec<u8>) -> Value {
    let path = format!(
        "/api/v1/sessions/{id}/{}",
        if kind == "image" { "images" } else { "files" }
    );
    let r = app
        .api
        .post(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .multipart(form(kind, name, bytes))
        .await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn media_upload_detects_real_format_and_serves_only_after_note_access() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let data = png();
    let m = upload(&app, id, "image", "anh.png", data.clone()).await;
    assert_eq!(m["mime_type"], "image/png");
    assert!(m.get("storage_key").is_none());
    let path = format!("/api/v1/sessions/{id}/images/{}", m["id"]);
    app.api.get(&path).await.assert_status_unauthorized();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    let r = app
        .api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.as_bytes(), data.as_slice());
    assert_eq!(r.header("x-content-type-options"), "nosniff");
    assert_eq!(r.header("content-type"), "image/png");
    let other_lesson = lesson(&app).await;
    app.api
        .get(&format!(
            "/api/v1/sessions/{other_lesson}/images/{}",
            m["id"]
        ))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
#[tokio::test]
async fn media_rejects_spoofed_types_sizes_and_unknown_multipart_fields() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/sessions/{id}/images");
    let auth = bearer(&app.owner_token);
    app.api
        .post(&path)
        .multipart(form("image", "test.png", png()))
        .await
        .assert_status_unauthorized();
    app.api
        .post(&path)
        .add_header("authorization", bearer(&app.other_token))
        .multipart(form("image", "test.png", png()))
        .await
        .assert_status_not_found();
    for f in [
        form("image", "fake.png", b"<svg>evil</svg>".to_vec()),
        form("wrong", "x.png", png()),
        form("image", "empty.png", vec![]),
        form("image", "big.png", vec![0; 5 * 1024 * 1024 + 1]),
        MultipartForm::new()
            .add_part("image", Part::bytes(png()).file_name("x.png"))
            .add_text("user_id", "1"),
    ] {
        app.api
            .post(&path)
            .add_header("authorization", &auth)
            .multipart(f)
            .await
            .assert_status_unprocessable_entity();
    }
    for (name, data) in [
        ("fake.pdf", b"Not PDF".to_vec()),
        ("fake.txt", png()),
        ("bad.exe", b"anything".to_vec()),
        ("fake.docx", b"PK\x03\x04word/".to_vec()),
    ] {
        app.api
            .post(&format!("/api/v1/sessions/{id}/files"))
            .add_header("authorization", &auth)
            .multipart(form("file", name, data))
            .await
            .assert_status_unprocessable_entity();
    }
    let mut wide = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(8193, 1)
        .write_to(&mut wide, image::ImageFormat::Png)
        .unwrap();
    app.api
        .post(&path)
        .add_header("authorization", &auth)
        .multipart(form("image", "wide.png", wide.into_inner()))
        .await
        .assert_status_unprocessable_entity();
    assert!(media::Entity::find()
        .filter(media::Column::SessionId.eq(id))
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    assert!(!std::path::Path::new(&app.media_root).exists());
    app.cleanup().await;
}
#[tokio::test]
async fn files_download_with_safe_filename_and_office_package_validation() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let body = "Bài học Jobs".as_bytes().to_vec();
    let m = upload(&app, id, "file", "../bài học.txt", body.clone()).await;
    assert_eq!(m["name"], ".._bài học.txt");
    let path = format!("/api/v1/sessions/{id}/files/{}", m["id"]);
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_ok();
    let r = app
        .api
        .get(&format!("{path}/download"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.as_bytes(), body.as_slice());
    assert!(r
        .header("content-disposition")
        .to_str()
        .unwrap()
        .starts_with("attachment;"));
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "[Content_Types].xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#).unwrap();
    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"<document/>").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let m = upload(&app, id, "file", "lesson.docx", bytes.clone()).await;
    assert_eq!(
        m["mime_type"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    app.api
        .post(&format!("/api/v1/sessions/{id}/files"))
        .add_header("authorization", bearer(&app.owner_token))
        .multipart(form("file", "lesson.xlsx", bytes))
        .await
        .assert_status_unprocessable_entity();
    app.cleanup().await;
}
#[tokio::test]
async fn note_media_refs_are_scoped_and_sharing_revocation_blocks_download() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let second = lesson(&app).await;
    let m = upload(&app, id, "image", "pic.png", png()).await;
    let body = json!({"content":{"type":"doc","content":[{"type":"image","attrs":{"mediaId":m["id"],"alt":"Jobs","align":"center","width":200,"captionVisible":true},"content":[{"type":"text","text":"Caption"}]}]},"schema_version":3,"base_revision":0});
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&body)
        .await
        .assert_status_ok();
    app.api
        .put(&format!("/api/v1/sessions/{second}/note"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&body)
        .await
        .assert_status_unprocessable_entity();
    let path = format!("/api/v1/sessions/{id}/images/{}", m["id"]);
    let sharing = format!("/api/v1/sessions/{id}/collaborators");
    let r = app
        .api
        .post(&sharing)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let user = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_ok();
    app.api
        .post(&format!("/api/v1/sessions/{id}/files"))
        .add_header("authorization", bearer(&app.other_token))
        .multipart(form("file", "shared.txt", b"Shared note".to_vec()))
        .await
        .assert_status(StatusCode::CREATED);
    app.api
        .delete(&format!("{sharing}/{user}"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_no_content();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .delete(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_no_content();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    app.cleanup().await;
}

#[tokio::test]
async fn storage_failure_does_not_create_a_media_record() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    tokio::fs::write(&app.media_root, b"not a directory")
        .await
        .unwrap();
    app.api
        .post(&format!("/api/v1/sessions/{id}/images"))
        .add_header("authorization", bearer(&app.owner_token))
        .multipart(form("image", "x.png", png()))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    assert!(media::Entity::find()
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    tokio::fs::remove_file(&app.media_root).await.unwrap();
    app.cleanup().await;
}
