use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::sessions;

fn bearer(app: &LearningApp) -> String {
    format!("Bearer {}", app.owner_token)
}
async fn parents(app: &LearningApp) -> (i64, i64) {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", bearer(app))
        .json(&json!({"name":"School"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let place = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .post(&format!("/api/v1/places/{place}/courses"))
        .add_header("authorization", bearer(app))
        .json(&json!({"name":"Foundation","starts_on":"2026-09-29","ends_on":"2027-03-31"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    (place, r.json::<Value>()["data"]["id"].as_i64().unwrap())
}
fn input() -> Value {
    json!({"title":" Lesson 1 ","starts_at":"2026-10-06T18:00:00+07:00","duration_minutes":90,"timezone":"Asia/Saigon"})
}
async fn create(app: &LearningApp, path: &str, body: Value) -> Value {
    let r = app
        .api
        .post(path)
        .add_header("authorization", bearer(app))
        .json(&body)
        .await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn sessions_crud_supports_independent_lessons_and_utc_instants() {
    let app = LearningApp::spawn().await;
    let (place, course) = parents(&app).await;
    let data = create(&app, &format!("/api/v1/courses/{course}/sessions"), input()).await;
    assert_eq!(data["title"], "Lesson 1");
    assert_eq!(data["starts_at"], "2026-10-06T11:00:00Z");
    assert_eq!(data["ends_at"], "2026-10-06T12:30:00Z");
    assert_eq!(data["status"], "scheduled");
    assert_eq!(data["place"], json!({"id":place,"name":"School"}));
    assert_eq!(data["course"], json!({"id":course,"name":"Foundation"}));
    let independent = create(&app, &format!("/api/v1/places/{place}/sessions"), input()).await;
    assert!(independent["course"].is_null());
    // Makeup lessons may be outside the initial course calendar.
    let mut makeup = input();
    makeup["starts_at"] = json!("2028-01-01T11:00:00Z");
    create(&app, &format!("/api/v1/courses/{course}/sessions"), makeup).await;
    let path = format!("/api/v1/sessions/{}", data["id"]);
    let r = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app))
        .json(&json!({"title":" Revised ","status":"cancelled"}))
        .await;
    r.assert_status_ok();
    let updated = r.json::<Value>()["data"].clone();
    assert_eq!(updated["title"], "Revised");
    assert_eq!(updated["status"], "cancelled");
    assert_eq!(updated["starts_at"], data["starts_at"]);
    app.api
        .delete(&path)
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_no_content();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_not_found();
    let row = sessions::Entity::find_by_id(data["id"].as_i64().unwrap())
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert!(row.deleted_at.is_some());
    app.cleanup().await;
}
#[tokio::test]
async fn sessions_require_bearer_and_hide_other_owners() {
    let app = LearningApp::spawn().await;
    let (place, course) = parents(&app).await;
    let collection = format!("/api/v1/courses/{course}/sessions");
    let data = create(&app, &collection, input()).await;
    let path = format!("/api/v1/sessions/{}", data["id"]);
    let other = format!("Bearer {}", app.other_token);
    for list in [collection, format!("/api/v1/places/{place}/sessions")] {
        app.api.get(&list).await.assert_status_unauthorized();
        app.api
            .post(&list)
            .json(&input())
            .await
            .assert_status_unauthorized();
        app.api
            .get(&list)
            .add_header("authorization", &other)
            .await
            .assert_status_not_found();
        app.api
            .post(&list)
            .add_header("authorization", &other)
            .json(&input())
            .await
            .assert_status_not_found();
    }
    app.api.get(&path).await.assert_status_unauthorized();
    app.api
        .patch(&path)
        .json(&json!({"title":"X"}))
        .await
        .assert_status_unauthorized();
    app.api.delete(&path).await.assert_status_unauthorized();
    app.api
        .get(&path)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .patch(&path)
        .add_header("authorization", &other)
        .json(&json!({"title":"X"}))
        .await
        .assert_status_not_found();
    app.api
        .delete(&path)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.cleanup().await;
}
#[tokio::test]
async fn sessions_validate_patch_time_and_measure_elapsed_duration_across_dst() {
    let app = LearningApp::spawn().await;
    let (_, course) = parents(&app).await;
    let list = format!("/api/v1/courses/{course}/sessions");
    for (key, value) in [
        ("timezone", json!("Mars/Olympus")),
        ("title", json!("  ")),
        ("duration_minutes", json!(0)),
        ("duration_minutes", json!(1441)),
        ("status", json!("unknown")),
        ("starts_at", json!("2026-10-06T18:00:00")),
        ("starts_at", json!("2026-10-06T23:00:00+07:00")),
        ("starts_at", json!("2026-10-06T11:00:00.000000001Z")),
        ("place_id", json!(1)),
    ] {
        let mut body = input();
        body[key] = value;
        app.api
            .post(&list)
            .add_header("authorization", bearer(&app))
            .json(&body)
            .await
            .assert_status_unprocessable_entity();
    }
    let data = create(&app, &list, input()).await;
    let path = format!("/api/v1/sessions/{}", data["id"]);
    for body in [
        json!({"duration_minutes":120}),
        json!({"starts_at":null}),
        json!({"duration_minutes":null}),
        json!({"title":null}),
        json!({"status":null}),
        json!({"course_id":course}),
        json!({"duration_minutes":500,"timezone":"Asia/Ho_Chi_Minh"}),
    ] {
        app.api
            .patch(&path)
            .add_header("authorization", bearer(&app))
            .json(&body)
            .await
            .assert_status_unprocessable_entity();
    }
    let r = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app))
        .json(&json!({"duration_minutes":120,"timezone":"Asia/Ho_Chi_Minh"}))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["ends_at"], "2026-10-06T13:00:00Z");
    let dst = create(&app, &list, json!({"title":"DST","starts_at":"2027-03-14T01:30:00-05:00","duration_minutes":90,"timezone":"America/New_York"})).await;
    assert_eq!(dst["starts_at"], "2027-03-14T06:30:00Z");
    assert_eq!(dst["ends_at"], "2027-03-14T08:00:00Z");
    app.cleanup().await;
}
#[tokio::test]
async fn sessions_list_paginates_each_parent_and_searches_literal_title() {
    let app = LearningApp::spawn().await;
    let (place, course) = parents(&app).await;
    let list = format!("/api/v1/courses/{course}/sessions");
    let mut late = input();
    late["starts_at"] = json!("2026-10-08T11:00:00Z");
    late["title"] = json!("100%_ Lesson");
    let last = create(&app, &list, late).await;
    let first = create(&app, &list, input()).await;
    create(&app, &format!("/api/v1/places/{place}/sessions"), input()).await;
    let r = app
        .api
        .get(&list)
        .add_header("authorization", bearer(&app))
        .add_query_param("per_page", 1)
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 2);
    assert_eq!(r.json::<Value>()["data"][0]["id"], first["id"]);
    let r = app
        .api
        .get(&list)
        .add_header("authorization", bearer(&app))
        .add_query_param("q", "%_")
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 1);
    assert_eq!(r.json::<Value>()["data"][0]["id"], last["id"]);
    let r = app
        .api
        .get(&format!("/api/v1/places/{place}/sessions"))
        .add_header("authorization", bearer(&app))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 1);
    app.cleanup().await;
}
#[tokio::test]
async fn sessions_soft_delete_cascades_and_database_rejects_mismatched_parent() {
    let app = LearningApp::spawn().await;
    let (place, course) = parents(&app).await;
    let (other_place, _) = parents(&app).await;
    let data = create(&app, &format!("/api/v1/courses/{course}/sessions"), input()).await;
    let id = data["id"].as_i64().unwrap();
    let row = sessions::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut invalid: sessions::ActiveModel = row.into();
    invalid.place_id = Set(other_place);
    assert!(invalid.update(app.db.as_ref()).await.is_err());
    let independent = create(&app, &format!("/api/v1/places/{place}/sessions"), input()).await;
    app.api
        .delete(&format!("/api/v1/courses/{course}"))
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_no_content();
    app.api
        .get(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_not_found();
    let deleted = sessions::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at;
    assert!(deleted.is_some());
    let independent_path = format!("/api/v1/sessions/{}", independent["id"]);
    app.api
        .get(&independent_path)
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_ok();
    app.api
        .delete(&format!("/api/v1/places/{place}"))
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_no_content();
    app.api
        .get(&independent_path)
        .add_header("authorization", bearer(&app))
        .await
        .assert_status_not_found();
    assert_eq!(
        sessions::Entity::find_by_id(id)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .deleted_at,
        deleted
    );
    app.cleanup().await;
}
#[tokio::test]
async fn concurrent_session_creation_and_course_deletion_leave_no_active_children() {
    let app = LearningApp::spawn().await;
    let (_, course) = parents(&app).await;
    let list = format!("/api/v1/courses/{course}/sessions");
    let path = format!("/api/v1/courses/{course}");
    let token = bearer(&app);
    let body = input();
    let (create, delete) = tokio::join!(
        app.api
            .post(&list)
            .add_header("authorization", &token)
            .json(&body),
        app.api.delete(&path).add_header("authorization", &token)
    );
    delete.assert_status_no_content();
    assert!(matches!(
        create.status_code(),
        StatusCode::CREATED | StatusCode::NOT_FOUND
    ));
    assert!(sessions::Entity::find()
        .filter(sessions::Column::CourseId.eq(course))
        .filter(sessions::Column::DeletedAt.is_null())
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    app.cleanup().await;
}
