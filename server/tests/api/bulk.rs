use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use server::entities::{courses, places, sessions};
fn bearer(s: &str) -> String {
    format!("Bearer {s}")
}
async fn hierarchy(app: &LearningApp, token: &str) -> (i64, i64, i64, i64) {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", bearer(token))
        .json(&json!({"name":"Bulk school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let p = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .post(&format!("/api/v1/places/{p}/courses"))
        .add_header("authorization", bearer(token))
        .json(&json!({"name":"Course","starts_on":"2026-10-01"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let c = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let mut ids = Vec::new();
    for path in [
        format!("/api/v1/courses/{c}/sessions"),
        format!("/api/v1/places/{p}/sessions"),
    ] {
        let r=app.api.post(&path).add_header("authorization",bearer(token)).json(&json!({"title":"Lesson","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"Asia/Ho_Chi_Minh"})).await;
        r.assert_status(StatusCode::CREATED);
        ids.push(r.json::<Value>()["data"]["id"].as_i64().unwrap());
    }
    (p, c, ids[0], ids[1])
}
#[tokio::test]
async fn bulk_delete_all_levels_is_atomic_and_cascades_soft_delete() {
    let app = LearningApp::spawn().await;
    let a = hierarchy(&app, &app.owner_token).await;
    let b = hierarchy(&app, &app.other_token).await;
    let auth = bearer(&app.owner_token);
    for (path, ids) in [
        ("/api/v1/places", vec![a.0, b.0]),
        ("/api/v1/courses", vec![a.1, b.1]),
        ("/api/v1/sessions", vec![a.2, b.2]),
    ] {
        app.api
            .delete(path)
            .add_header("authorization", &auth)
            .json(&json!({"ids":ids}))
            .await
            .assert_status_not_found();
    }
    assert!(places::Entity::find_by_id(a.0)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    assert!(courses::Entity::find_by_id(a.1)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    assert!(sessions::Entity::find_by_id(a.2)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    app.api
        .delete("/api/v1/sessions")
        .add_header("authorization", &auth)
        .json(&json!({"ids":[a.2,a.3]}))
        .await
        .assert_status_no_content();
    let original = sessions::Entity::find_by_id(a.2)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at;
    assert!(original.is_some());
    app.api
        .delete("/api/v1/places")
        .add_header("authorization", &auth)
        .json(&json!({"ids":[a.0]}))
        .await
        .assert_status_no_content();
    assert_eq!(
        sessions::Entity::find_by_id(a.2)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .deleted_at,
        original
    );
    assert!(courses::Entity::find_by_id(a.1)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    let c = hierarchy(&app, &app.owner_token).await;
    app.api
        .delete("/api/v1/courses")
        .add_header("authorization", &auth)
        .json(&json!({"ids":[c.1]}))
        .await
        .assert_status_no_content();
    assert!(sessions::Entity::find_by_id(c.2)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    assert!(sessions::Entity::find_by_id(c.3)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    app.cleanup().await;
}
#[tokio::test]
async fn bulk_rejects_invalid_sets_and_deleted_or_missing_ids_without_partial_updates() {
    let app = LearningApp::spawn().await;
    let a = hierarchy(&app, &app.owner_token).await;
    let auth = bearer(&app.owner_token);
    app.api
        .delete("/api/v1/sessions")
        .json(&json!({"ids":[a.2]}))
        .await
        .assert_status_unauthorized();
    for ids in [
        vec![],
        vec![a.2, a.2],
        vec![0],
        vec![-1],
        (1..=101).collect(),
    ] {
        app.api
            .delete("/api/v1/sessions")
            .add_header("authorization", &auth)
            .json(&json!({"ids":ids}))
            .await
            .assert_status_unprocessable_entity();
    }
    app.api
        .delete("/api/v1/sessions")
        .add_header("authorization", &auth)
        .json(&json!({"ids":[a.2,999999]}))
        .await
        .assert_status_not_found();
    app.api
        .delete(&format!("/api/v1/sessions/{}", a.3))
        .add_header("authorization", &auth)
        .await
        .assert_status_no_content();
    app.api
        .delete("/api/v1/sessions")
        .add_header("authorization", &auth)
        .json(&json!({"ids":[a.2,a.3]}))
        .await
        .assert_status_not_found();
    assert!(sessions::Entity::find_by_id(a.2)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    app.cleanup().await;
}

#[tokio::test]
async fn concurrent_bulk_requests_lock_parents_in_the_same_order() {
    let app = LearningApp::spawn().await;
    let a = hierarchy(&app, &app.owner_token).await;
    let b = hierarchy(&app, &app.owner_token).await;
    let auth = bearer(&app.owner_token);
    let first = json!({"ids":[a.2,b.2]});
    let reverse = json!({"ids":[b.2,a.2]});
    let (a, b) = tokio::join!(
        app.api
            .delete("/api/v1/sessions")
            .add_header("authorization", &auth)
            .json(&first),
        app.api
            .delete("/api/v1/sessions")
            .add_header("authorization", &auth)
            .json(&reverse)
    );
    let mut statuses = [a.status_code().as_u16(), b.status_code().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [204, 404]);
    app.cleanup().await;
}
