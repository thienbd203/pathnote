use super::learning_helpers::LearningApp;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use server::entities::{courses, places};
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
async fn place(app: &LearningApp, token: &str, name: &str) -> i64 {
    let response = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", bearer(token))
        .json(&json!({"name":name}))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    response.json::<Value>()["data"]["id"].as_i64().unwrap()
}
fn input() -> Value {
    json!({"name":" Foundation ","description":" Course description ","starts_on":"2026-09-29","ends_on":"2027-03-31"})
}
async fn course(app: &LearningApp, place_id: i64, body: Value) -> i64 {
    let response = app
        .api
        .post(&format!("/api/v1/places/{place_id}/courses"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&body)
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    response.json::<Value>()["data"]["id"].as_i64().unwrap()
}
#[tokio::test]
async fn courses_crud_join_owner_and_parent_metadata() {
    let app = LearningApp::spawn().await;
    let parent = place(&app, &app.owner_token, "School").await;
    let foreign = place(&app, &app.other_token, "Other school").await;
    let list = format!("/api/v1/places/{parent}/courses");
    app.api.get(&list).await.assert_status_unauthorized();
    app.api
        .post(&list)
        .json(&input())
        .await
        .assert_status_unauthorized();
    let id = course(&app, parent, input()).await;
    let path = format!("/api/v1/courses/{id}");
    let response = app
        .api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    let data = response.json::<Value>()["data"].clone();
    assert_eq!(data["name"], "Foundation");
    assert_eq!(data["description"], "Course description");
    assert_eq!(data["starts_on"], "2026-09-29");
    assert_eq!(data["place"], json!({"id":parent,"name":"School"}));
    assert!(data.get("user_id").is_none());
    assert!(data.get("place_id").is_none());
    let other = bearer(&app.other_token);
    app.api
        .get(&path)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .patch(&path)
        .add_header("authorization", &other)
        .json(&json!({"name":"Stolen"}))
        .await
        .assert_status_not_found();
    app.api
        .delete(&path)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .get(&list)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .post(&format!("/api/v1/places/{foreign}/courses"))
        .add_header("authorization", bearer(&app.owner_token))
        .json(&input())
        .await
        .assert_status_not_found();
    app.api
        .delete(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_no_content();
    app.api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    assert!(courses::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    app.cleanup().await;
}
#[tokio::test]
async fn courses_partial_dates_compare_existing_values_and_allow_explicit_null_end() {
    let app = LearningApp::spawn().await;
    let parent = place(&app, &app.owner_token, "School").await;
    let id = course(&app, parent, input()).await;
    let path = format!("/api/v1/courses/{id}");
    let token = bearer(&app.owner_token);
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", &token)
        .json(&json!({"name":"New name"}))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["starts_on"], "2026-09-29");
    assert_eq!(
        response.json::<Value>()["data"]["description"],
        "Course description"
    );
    for body in [
        json!({"starts_on":"2027-04-01"}),
        json!({"ends_on":"2026-09-28"}),
        json!({"starts_on":null}),
        json!({"name":null}),
        json!({"place_id":parent}),
    ] {
        app.api
            .patch(&path)
            .add_header("authorization", &token)
            .json(&body)
            .await
            .assert_status_unprocessable_entity();
    }
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", &token)
        .json(&json!({"ends_on":null,"description":null}))
        .await;
    response.assert_status_ok();
    assert!(response.json::<Value>()["data"]["ends_on"].is_null());
    assert!(response.json::<Value>()["data"]["description"].is_null());
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", &token)
        .json(&json!({"starts_on":"2027-04-01"}))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["starts_on"], "2027-04-01");
    app.cleanup().await;
}
#[tokio::test]
async fn courses_validate_dates_before_persisting_and_paginate_selected_parent_only() {
    let app = LearningApp::spawn().await;
    let first = place(&app, &app.owner_token, "First").await;
    let second = place(&app, &app.owner_token, "Second").await;
    let path = format!("/api/v1/places/{first}/courses");
    let token = bearer(&app.owner_token);
    for body in [
        json!({"name":"Course","starts_on":"2026-02-30"}),
        json!({"name":"Course","starts_on":"2026-09-29","ends_on":"2026-09-28"}),
        json!({"name":"   ","starts_on":"2026-09-29"}),
        json!({"name":"Course","starts_on":"2026-09-29","user_id":app.owner_id}),
        json!({"name":"Course","starts_on":"2026-09-29","schedule":{}}),
    ] {
        app.api
            .post(&path)
            .add_header("authorization", &token)
            .json(&body)
            .await
            .assert_status_unprocessable_entity();
    }
    let a = course(
        &app,
        first,
        json!({"name":"Alpha","starts_on":"2026-09-29"}),
    )
    .await;
    let b = course(
        &app,
        first,
        json!({"name":"100%_ School","starts_on":"2026-09-29"}),
    )
    .await;
    course(&app, second, input()).await;
    let response = app
        .api
        .get(&path)
        .add_query_param("per_page", 1)
        .add_query_param("page", 2)
        .add_header("authorization", &token)
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"][0]["id"], b);
    assert_eq!(response.json::<Value>()["meta"]["total"], 2);
    let response = app
        .api
        .get(&path)
        .add_query_param("q", "ALPHA")
        .add_header("authorization", &token)
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"][0]["id"], a);
    assert_eq!(response.json::<Value>()["meta"]["total"], 1);
    let response = app
        .api
        .get(&path)
        .add_query_param("q", "%_")
        .add_header("authorization", &token)
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 1);
    // Database CHECK protects the invariant even if a future caller bypasses DTO validation.
    let current = courses::Entity::find_by_id(a)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let mut invalid: courses::ActiveModel = current.into();
    invalid.ends_on = Set(Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));
    let result = invalid.update(app.db.as_ref()).await;
    assert!(result.is_err());
    app.cleanup().await;
}
#[tokio::test]
async fn deleting_place_soft_deletes_courses_and_hides_direct_and_nested_routes() {
    let app = LearningApp::spawn().await;
    let parent = place(&app, &app.owner_token, "School").await;
    let id = course(&app, parent, input()).await;
    let token = bearer(&app.owner_token);
    app.api
        .delete(&format!("/api/v1/places/{parent}"))
        .add_header("authorization", &token)
        .await
        .assert_status_no_content();
    app.api
        .get(&format!("/api/v1/courses/{id}"))
        .add_header("authorization", &token)
        .await
        .assert_status_not_found();
    app.api
        .get(&format!("/api/v1/places/{parent}/courses"))
        .add_header("authorization", &token)
        .await
        .assert_status_not_found();
    app.api
        .post(&format!("/api/v1/places/{parent}/courses"))
        .add_header("authorization", &token)
        .json(&input())
        .await
        .assert_status_not_found();
    let row = courses::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert!(row.deleted_at.is_some());
    assert!(places::Entity::find_by_id(parent)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    app.cleanup().await;
}
#[tokio::test]
async fn concurrent_course_creation_and_place_deletion_leave_no_active_children() {
    let app = LearningApp::spawn().await;
    let parent = place(&app, &app.owner_token, "School").await;
    let token = bearer(&app.owner_token);
    let create_path = format!("/api/v1/places/{parent}/courses");
    let delete_path = format!("/api/v1/places/{parent}");
    let body = input();
    let (create, delete) = tokio::join!(
        app.api
            .post(&create_path)
            .add_header("authorization", &token)
            .json(&body),
        app.api
            .delete(&delete_path)
            .add_header("authorization", &token)
    );
    delete.assert_status_no_content();
    assert!(matches!(
        create.status_code(),
        axum::http::StatusCode::CREATED | axum::http::StatusCode::NOT_FOUND
    ));
    let active = courses::Entity::find()
        .filter(courses::Column::PlaceId.eq(parent))
        .filter(courses::Column::DeletedAt.is_null())
        .all(app.db.as_ref())
        .await
        .unwrap();
    assert!(active.is_empty());
    app.cleanup().await;
}

#[tokio::test]
async fn library_stats_and_course_decks_follow_ownership_and_current_notes() {
    use super::vocabulary::{doc, entry, save};
    let app = LearningApp::spawn().await;
    let p = place(&app, &app.owner_token, "Library").await;
    let c = course(&app, p, input()).await;
    let other = course(&app, p, input()).await;
    let mut ids = Vec::new();
    for (course_id, word) in [(c, "teacher"), (other, "doctor")] {
        let response = app.api.post(&format!("/api/v1/courses/{course_id}/sessions"))
            .add_header("authorization", bearer(&app.owner_token))
            .json(&json!({"title":"Lesson","starts_at":"2026-10-15T11:00:00Z","duration_minutes":90,"timezone":"Asia/Ho_Chi_Minh"})).await;
        response.assert_status(axum::http::StatusCode::CREATED);
        let id = response.json::<Value>()["data"]["id"].as_i64().unwrap();
        save(
            &app,
            id,
            &doc(vec![entry(word, "nghĩa", &[]), entry("", "", &[])]),
            0,
        )
        .await;
        ids.push(id);
    }
    let empty = course(&app, p, input()).await;
    let bulk_url = format!("/api/v1/courses/library-stats?ids={other},{c},{empty},{c},999999");
    let bulk = app
        .api
        .get(&bulk_url)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    bulk.assert_status_ok();
    let summary = bulk.json::<Value>()["data"].clone();
    assert_eq!(summary.as_array().unwrap().len(), 3);
    assert_eq!(summary[0]["id"], other);
    assert_eq!(summary[1]["id"], c);
    assert_eq!(summary[1]["words"], 1);
    assert_eq!(summary[1]["session_count"], 1);
    assert_eq!(summary[1]["latest_past_session"]["id"], ids[0]);
    assert!(summary[1].get("sessions").is_none());
    assert_eq!(summary[2]["session_count"], 0);
    assert!(summary[2]["latest_past_session"].is_null());
    let foreign = app
        .api
        .get(&bulk_url)
        .add_header("authorization", bearer(&app.other_token))
        .await;
    foreign.assert_status_ok();
    assert_eq!(foreign.json::<Value>()["data"], json!([]));
    for invalid in [
        "0".to_string(),
        "1,,2".into(),
        vec![c.to_string(); 101].join(","),
    ] {
        app.api
            .get(&format!("/api/v1/courses/library-stats?ids={invalid}"))
            .add_header("authorization", bearer(&app.owner_token))
            .await
            .assert_status(axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    let url = format!("/api/v1/courses/{c}/library-stats");
    let response = app
        .api
        .get(&url)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>()["data"],
        json!({"words":1,"exercises":0,"session_count":1,"latest_past_session":{"id":ids[0],"title":"Lesson","starts_at":"2026-10-15T11:00:00Z"},"sessions":[{"id":ids[0],"words":1,"exercises":0}]})
    );
    app.api
        .get(&url)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    let response = app
        .api
        .get(&format!("/api/v1/vocabulary?course_id={c}"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["word_count"], 1);
    let response = app
        .api
        .post("/api/v1/flashcards/next")
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"course_id":c}))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["overview"]["new"], 1);
    app.api
        .get(&format!("/api/v1/flashcards?course_id={c}"))
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .get(&format!("/api/v1/flashcards/history?course_id={c}"))
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_status_not_found();
    app.api
        .get("/api/v1/flashcards?course_id=0")
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status(axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    let mut exercise_ids = Vec::new();
    for _ in 0..2 {
        let response = app.api.post(&format!("/api/v1/sessions/{}/exercises",ids[0]))
            .add_header("authorization",bearer(&app.owner_token))
            .json(&json!({"title":"Test","questions":[{"type":"choice","prompt":"Pick one","options":["a","b"],"answers":["a"],"explanation":"","caseSensitive":false}]})).await;
        response.assert_status(axum::http::StatusCode::CREATED);
        exercise_ids.push(response.json::<Value>()["data"]["id"].as_i64().unwrap());
    }
    let mut content = doc(vec![entry("teacher", "nghĩa", &[])]);
    content["content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"exercise","attrs":{"exerciseId":exercise_ids[0]}}));
    save(&app, ids[0], &content, 1).await;
    let response = app
        .api
        .get(&url)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["exercises"], 1);
    save(
        &app,
        ids[0],
        &json!({"type":"doc","content":[{"type":"paragraph"}]}),
        2,
    )
    .await;
    let response = app
        .api
        .get(&url)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(response.json::<Value>()["data"]["words"], 0);
    app.api
        .delete(&format!("/api/v1/sessions/{}", ids[0]))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_no_content();
    let response = app
        .api
        .get(&url)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(response.json::<Value>()["data"]["sessions"], json!([]));
    app.cleanup().await;
}
