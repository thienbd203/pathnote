use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, save},
};
use axum::http::StatusCode;
use sea_orm::{EntityTrait, Set};
use serde_json::{json, Value};
use server::entities::user_settings;

#[tokio::test]
async fn settings_are_personal_persistent_and_do_not_rewrite_lessons_or_other_categories() {
    let app = LearningApp::spawn().await;
    let auth = format!("Bearer {}", app.owner_token);
    let other = format!("Bearer {}", app.other_token);
    let read = app
        .api
        .get("/api/v1/settings")
        .add_header("authorization", &auth)
        .await;
    read.assert_status_ok();
    assert_eq!(
        read.json::<Value>()["data"]["learning"],
        json!({"session_start_time":"19:30","session_duration_minutes":60})
    );
    let id = lesson(&app, &app.owner_token).await;
    save(&app, id, &doc(vec![entry("teacher", "giáo viên", &[])]), 0).await;
    let before = app
        .api
        .get(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", &auth)
        .await
        .json::<Value>();
    user_settings::Entity::insert(user_settings::ActiveModel {
        user_id: Set(app.owner_id),
        category: Set("future_category".into()),
        value: Set(json!({"keep":true})),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    })
    .exec(app.db.as_ref())
    .await
    .unwrap();
    for start in ["18:00", "20:00"] {
        let input = json!({"session_start_time":start,"session_duration_minutes":90});
        let response = app
            .api
            .put("/api/v1/settings/learning")
            .add_header("authorization", &auth)
            .json(&input)
            .await;
        response.assert_status_ok();
        let result = app
            .api
            .get("/api/v1/settings")
            .add_header("authorization", &auth)
            .await
            .json::<Value>();
        assert_eq!(result["data"]["learning"], input);
    }
    let untouched = app
        .api
        .get("/api/v1/settings")
        .add_header("authorization", &other)
        .await
        .json::<Value>();
    assert_eq!(untouched["data"]["learning"]["session_start_time"], "19:30");
    let after = app
        .api
        .get(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", &auth)
        .await
        .json::<Value>();
    assert_eq!(before, after);
    assert_eq!(
        user_settings::Entity::find_by_id((app.owner_id, "future_category".into()))
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap()
            .value,
        json!({"keep":true})
    );
    app.cleanup().await;
}

#[tokio::test]
async fn settings_require_bearer_and_reject_invalid_or_injected_preferences() {
    let app = LearningApp::spawn().await;
    app.api
        .get("/api/v1/settings")
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    app.api
        .put("/api/v1/settings/learning")
        .json(&json!({"session_start_time":"18:00","session_duration_minutes":90}))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    for input in [
        json!({"session_start_time":"24:00","session_duration_minutes":90}),
        json!({"session_start_time":"8:00","session_duration_minutes":90}),
        json!({"session_start_time":"18:00","session_duration_minutes":0}),
        json!({"session_start_time":"18:00","session_duration_minutes":90.5}),
        json!({"session_start_time":"23:00","session_duration_minutes":60}),
        json!({"session_start_time":"18:00","session_duration_minutes":90,"user_id":1}),
        json!({"session_start_time":null,"session_duration_minutes":90}),
    ] {
        app.api
            .put("/api/v1/settings/learning")
            .add_header("authorization", format!("Bearer {}", app.owner_token))
            .json(&input)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert!(user_settings::Entity::find()
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    app.cleanup().await;
}

#[tokio::test]
async fn navigation_chips_are_personal_persistent_and_do_not_change_learning_defaults() {
    let app = LearningApp::spawn().await;
    let owner = format!("Bearer {}", app.owner_token);
    let other = format!("Bearer {}", app.other_token);
    app.api
        .get("/api/v1/settings/navigation")
        .await
        .assert_status_unauthorized();
    let empty = json!({"hidden_chips":[],"pinned_places":[]});
    let r = app
        .api
        .get("/api/v1/settings/navigation")
        .add_header("authorization", owner.clone())
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"], empty);
    let input = json!({"hidden_chips":[{"kind":"course","id":2},{"kind":"place","id":3}],"pinned_places":[3,4]});
    app.api
        .put("/api/v1/settings/navigation")
        .add_header("authorization", owner.clone())
        .json(&input)
        .await
        .assert_status_ok();
    let r = app
        .api
        .get("/api/v1/settings/navigation")
        .add_header("authorization", owner.clone())
        .await;
    assert_eq!(r.json::<Value>()["data"], input);
    let r = app
        .api
        .get("/api/v1/settings/navigation")
        .add_header("authorization", other)
        .await;
    assert_eq!(r.json::<Value>()["data"], empty);
    let r = app
        .api
        .get("/api/v1/settings")
        .add_header("authorization", owner.clone())
        .await;
    assert_eq!(
        r.json::<Value>()["data"]["learning"]["session_start_time"],
        "19:30"
    );
    for invalid in [
        json!({"hidden_chips":[{"kind":"course","id":0}],"pinned_places":[]}),
        json!({"hidden_chips":[{"kind":"course","id":2},{"kind":"course","id":2}],"pinned_places":[]}),
        json!({"hidden_chips":[],"pinned_places":[1,1]}),
        json!({"hidden_chips":[{"kind":"unknown","id":1}],"pinned_places":[]}),
    ] {
        app.api
            .put("/api/v1/settings/navigation")
            .add_header("authorization", owner.clone())
            .json(&invalid)
            .await
            .assert_status_unprocessable_entity();
    }
    app.api
        .put("/api/v1/settings/navigation")
        .add_header("authorization", owner.clone())
        .json(&empty)
        .await
        .assert_status_ok();
    let r = app
        .api
        .get("/api/v1/settings/navigation")
        .add_header("authorization", owner)
        .await;
    assert_eq!(r.json::<Value>()["data"], empty);
    app.cleanup().await;
}
