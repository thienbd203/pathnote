use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn creation_initializes_notes_atomically_and_structure_does_not_copy_content_or_ids() {
    let app = LearningApp::spawn().await;
    let auth = format!("Bearer {}", app.owner_token);
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", auth.clone())
        .json(&json!({"name":"School"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let place = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .post(&format!("/api/v1/places/{place}/courses"))
        .add_header("authorization", auth.clone())
        .json(&json!({"name":"Foundation","starts_on":"2026-10-01"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let course = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let url = format!("/api/v1/courses/{course}/sessions");
    let base = json!({"title":"New lesson","starts_at":"2026-10-15T11:00:00Z","duration_minutes":90,"timezone":"Asia/Ho_Chi_Minh"});
    let mut input = base.clone();
    input["initial_note"] = json!({"mode":"lesson_template"});
    let r = app
        .api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&input)
        .await;
    r.assert_status(StatusCode::CREATED);
    let source_id = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{source_id}/note"))
        .add_header("authorization", auth.clone())
        .await;
    r.assert_status_ok();
    let initial = r.json::<Value>()["data"].clone();
    assert_eq!(initial["revision"], 1);
    assert_eq!(initial["content"]["content"][3]["type"], "vocabulary");
    let original_id = initial["content"]["content"][3]["content"][0]["attrs"]["id"]
        .as_str()
        .unwrap();
    let mut content = initial["content"].clone();
    content["content"][1] =
        json!({"type":"paragraph","content":[{"type":"text","text":"Private old answer"}]});
    app.api
        .put(&format!("/api/v1/sessions/{source_id}/note"))
        .add_header("authorization", auth.clone())
        .json(&json!({"content":content,"schema_version":4,"base_revision":1}))
        .await
        .assert_status_ok();
    input["initial_note"] = json!({"mode":"structure","session_id":source_id});
    let r = app
        .api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&input)
        .await;
    r.assert_status(StatusCode::CREATED);
    let copied = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{copied}/note"))
        .add_header("authorization", auth.clone())
        .await;
    r.assert_status_ok();
    let copy = r.json::<Value>()["data"]["content"].clone();
    assert_eq!(copy["content"][0]["content"][0]["text"], "Ghi chú");
    assert!(!copy.to_string().contains("Private old answer"));
    assert!(!copy.to_string().contains(original_id));
    let r = app
        .api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&base)
        .await;
    r.assert_status(StatusCode::CREATED);
    let blank = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .get(&format!("/api/v1/sessions/{blank}/note"))
        .add_header("authorization", auth.clone())
        .await;
    assert_eq!(r.json::<Value>()["data"]["revision"], 0);
    input["initial_note"] = json!({"mode":"structure","session_id":99999999});
    app.api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&input)
        .await
        .assert_status_not_found();
    input["initial_note"] = json!({"mode":"unsupported"});
    app.api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&input)
        .await
        .assert_status_unprocessable_entity();
    let r = app
        .api
        .get(&url)
        .add_header("authorization", auth.clone())
        .await;
    assert_eq!(r.json::<Value>()["meta"]["total"], 3);
    input["initial_note"] = json!({"mode":"structure","session_id":source_id});
    app.api
        .post(&url)
        .add_header("authorization", format!("Bearer {}", app.other_token))
        .json(&input)
        .await
        .assert_status_not_found();
    let r = app
        .api
        .post(&format!("/api/v1/places/{place}/sessions"))
        .add_header("authorization", auth.clone())
        .json(&input)
        .await;
    r.assert_status_unprocessable_entity();
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", format!("Bearer {}", app.other_token))
        .json(&json!({"name":"Private school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let other_place = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .post(&format!("/api/v1/places/{other_place}/sessions"))
        .add_header("authorization", format!("Bearer {}", app.other_token))
        .json(&base)
        .await;
    r.assert_status(StatusCode::CREATED);
    input["initial_note"] =
        json!({"mode":"structure","session_id":r.json::<Value>()["data"]["id"]});
    app.api
        .post(&url)
        .add_header("authorization", auth.clone())
        .json(&input)
        .await
        .assert_status_not_found();
    let r = app
        .api
        .get(&url)
        .add_header("authorization", auth.clone())
        .await;
    assert_eq!(r.json::<Value>()["meta"]["total"], 3);
    app.cleanup().await;
}
