use super::learning_helpers::LearningApp;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use server::entities::places;
const URL: &str = "/api/v1/places";
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
async fn create(app: &LearningApp, name: &str) -> i64 {
    let response = app
        .api
        .post(URL)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":name,"description":"description"}))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    response.json::<Value>()["data"]["id"].as_i64().unwrap()
}
#[tokio::test]
async fn places_crud_uses_bearer_and_scope_not_session_cookie_or_origin() {
    let app = LearningApp::spawn().await;
    app.api.get(URL).await.assert_status_unauthorized();
    app.api
        .post(URL)
        .json(&json!({"name":"School"}))
        .await
        .assert_status_unauthorized();
    let id = create(&app, "  School  ").await;
    let path = format!("{URL}/{id}");
    let response = app
        .api
        .get(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["name"], "School");
    assert!(response.json::<Value>()["data"].get("user_id").is_none());
    let row = places::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.user_id, app.owner_id);
    for token in [&app.other_token] {
        app.api
            .get(&path)
            .add_header("authorization", bearer(token))
            .await
            .assert_status_not_found();
        app.api
            .patch(&path)
            .add_header("authorization", bearer(token))
            .json(&json!({"name":"Stolen"}))
            .await
            .assert_status_not_found();
        app.api
            .delete(&path)
            .add_header("authorization", bearer(token))
            .await
            .assert_status_not_found();
    }
    app.api
        .get(URL)
        .add_header("authorization", bearer(&app.other_token))
        .await
        .assert_json(
            &json!({"data":[],"meta":{"current_page":1,"last_page":1,"per_page":25,"total":0}}),
        );
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
    app.api
        .delete(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_not_found();
    assert!(places::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    app.cleanup().await;
}
#[tokio::test]
async fn places_patch_distinguishes_missing_null_and_value() {
    let app = LearningApp::spawn().await;
    let id = create(&app, "First").await;
    let path = format!("{URL}/{id}");
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":" Second "}))
        .await;
    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>()["data"]["description"],
        "description"
    );
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"description":null}))
        .await;
    response.assert_status_ok();
    assert!(response.json::<Value>()["data"]["description"].is_null());
    app.api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"name":null}))
        .await
        .assert_status_unprocessable_entity();
    let response = app
        .api
        .patch(&path)
        .add_header("authorization", bearer(&app.owner_token))
        .json(&json!({"description":"  New description  "}))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"]["name"], "Second");
    assert_eq!(
        response.json::<Value>()["data"]["description"],
        "New description"
    );
    app.cleanup().await;
}
#[tokio::test]
async fn places_validate_input_and_reject_ownership_injection() {
    let app = LearningApp::spawn().await;
    for body in [
        json!({"name":"   "}),
        json!({"name":"a".repeat(201)}),
        json!({"name":"Valid","description":"a".repeat(2001)}),
        json!({"name":"Valid","user_id":app.owner_id}),
        json!({}),
        json!({"name":42}),
    ] {
        let response = app
            .api
            .post(URL)
            .add_header("authorization", bearer(&app.owner_token))
            .json(&body)
            .await;
        response.assert_status_unprocessable_entity();
        assert!(response.json::<Value>()["errors"].is_object());
        assert!(response.json::<Value>().get("success").is_none());
    }
    let id = create(&app, "Same name").await;
    assert_ne!(id, create(&app, "Same name").await);
    for query in ["page=0", "per_page=0", "per_page=101", "page=word"] {
        app.api
            .get(&format!("{URL}?{query}"))
            .add_header("authorization", bearer(&app.owner_token))
            .await
            .assert_status_unprocessable_entity();
    }
    app.cleanup().await;
}
#[tokio::test]
async fn places_paginate_search_literal_wildcards_and_exclude_deleted_records() {
    let app = LearningApp::spawn().await;
    let first = create(&app, "Alpha").await;
    let second = create(&app, "100%_ School").await;
    create(&app, "alpha other").await;
    let response = app
        .api
        .get(&format!("{URL}?page=2&per_page=1"))
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["data"][0]["id"], second);
    assert_eq!(response.json::<Value>()["meta"]["total"], 3);
    let response = app
        .api
        .get(URL)
        .add_query_param("q", "ALPHA")
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 2);
    let response = app
        .api
        .get(URL)
        .add_query_param("q", "%_")
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 1);
    assert_eq!(response.json::<Value>()["data"][0]["id"], second);
    app.api
        .delete(&format!("{URL}/{first}"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_status_no_content();
    let response = app
        .api
        .get(URL)
        .add_query_param("q", "alpha")
        .add_header("authorization", bearer(&app.owner_token))
        .await;
    assert_eq!(response.json::<Value>()["meta"]["total"], 1);
    assert_eq!(
        places::Entity::find()
            .filter(places::Column::DeletedAt.is_not_null())
            .all(app.db.as_ref())
            .await
            .unwrap()
            .len(),
        1
    );
    app.api
        .get(&format!("{URL}?page=9"))
        .add_header("authorization", bearer(&app.owner_token))
        .await
        .assert_json(
            &json!({"data":[],"meta":{"current_page":9,"last_page":1,"per_page":25,"total":2}}),
        );
    app.cleanup().await;
}
