use super::{
    learning_helpers::LearningApp,
    vocabulary::{doc, entry, lesson, library, save},
};
use axum::http::StatusCode;
use serde_json::{json, Value};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
async fn create(app: &LearningApp, token: &str, ids: Vec<i64>) -> Value {
    let response = app
        .api
        .post("/api/v1/word-groups")
        .add_header("authorization", auth(token))
        .json(&json!({"name":" Lo sợ ","note":"afraid of + V-ing\nGhi chú cá nhân","card_ids":ids}))
        .await;
    response.assert_status(StatusCode::CREATED);
    response.json::<Value>()["data"].clone()
}
#[tokio::test]
async fn groups_persist_live_source_ipa_order_notes_and_delete_only_memberships() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    let mut first = entry("afraid", "lo sợ", &[]);
    first["attrs"]["ipaUS"] = json!("/əˈfreɪd/");
    first["attrs"]["ipaUK"] = json!("/UK/");
    let second = entry("fearful", "sợ hãi", &[]);
    let mut note = doc(vec![first, second]);
    save(&app, session, &note, 0).await;
    let data = library(&app, &app.owner_token, "").await;
    let ids: Vec<_> = data["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_i64().unwrap())
        .collect();
    let group = create(&app, &app.owner_token, vec![ids[1], ids[0]]).await;
    let id = group["id"].as_i64().unwrap();
    assert_eq!(group["name"], "Lo sợ");
    assert_eq!(group["members"][0]["card_id"], ids[1]);
    assert_eq!(group["members"][1]["card"]["ipa"], "/əˈfreɪd/");
    assert_eq!(group["members"][1]["card"]["locale"], "en-US");
    note["content"][0]["content"][0]["content"][0]["attrs"]["ipaUS"] = json!("/new-US/");
    save(&app, session, &note, 1).await;
    let path = format!("/api/v1/word-groups/{id}");
    let current = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>()["data"]
        .clone();
    assert_eq!(current["members"][1]["card"]["ipa"], "/new-US/");
    assert_eq!(current["note"], group["note"]);
    let saved = app
        .api
        .put(&path)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"name":"Đã đổi tên","note":"","card_ids":[ids[0]],"revision":1}))
        .await;
    saved.assert_status_ok();
    assert_eq!(saved.json::<Value>()["data"]["revision"], 2);
    app.api
        .put(&path)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"name":"stale","note":"bad","card_ids":[],"revision":1}))
        .await
        .assert_status(StatusCode::CONFLICT);
    app.api
        .delete(&format!("{path}?revision=1"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::CONFLICT);
    app.api
        .delete(&format!("{path}?revision=2"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        library(&app, &app.owner_token, "").await["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    app.api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.cleanup().await;
}
#[tokio::test]
async fn groups_reject_cross_account_ids_and_search_only_accessible_active_words() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    save(
        &app,
        session,
        &doc(vec![
            entry("Afraid", "sợ hãi", &[]),
            entry("100%fear", "lo lắng", &[]),
        ]),
        0,
    )
    .await;
    let data = library(&app, &app.owner_token, "").await;
    let id = data["data"][0]["id"].as_i64().unwrap();
    let group = create(&app, &app.owner_token, vec![id]).await;
    let path = format!("/api/v1/word-groups/{}", group["id"]);
    for endpoint in [&path, "/api/v1/word-groups"] {
        let response = app
            .api
            .get(endpoint)
            .add_header("authorization", auth(&app.other_token))
            .await;
        if endpoint == path {
            response.assert_status(StatusCode::NOT_FOUND);
        } else {
            assert_eq!(response.json::<Value>()["data"], json!([]));
        }
    }
    app.api
        .post("/api/v1/word-groups")
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"name":"hack","note":"","card_ids":[id]}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.api
        .put(&path)
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"name":"hack","note":"","card_ids":[],"revision":1}))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.api
        .delete(&format!("{path}?revision=1"))
        .add_header("authorization", auth(&app.other_token))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    for (q, count) in [("afraid", 1), ("sợ", 1), ("%", 1)] {
        let response = app
            .api
            .get("/api/v1/word-groups/search")
            .add_header("authorization", auth(&app.owner_token))
            .add_query_param("q", q)
            .await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["meta"]["total"], count);
    }
    let other = app
        .api
        .get("/api/v1/word-groups/search?q=fear")
        .add_header("authorization", auth(&app.other_token))
        .await;
    assert_eq!(other.json::<Value>()["data"], json!([]));
    app.api
        .get("/api/v1/word-groups")
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    app.cleanup().await;
}
#[tokio::test]
async fn unavailable_source_is_redacted_but_can_be_kept_or_removed_and_undo_reactivates() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    let note = doc(vec![entry("afraid", "lo sợ", &[])]);
    save(&app, session, &note, 0).await;
    let data = library(&app, &app.owner_token, "").await;
    let card = data["data"][0]["id"].as_i64().unwrap();
    let group = create(&app, &app.owner_token, vec![card]).await;
    let path = format!("/api/v1/word-groups/{}", group["id"]);
    save(
        &app,
        session,
        &json!({"type":"doc","content":[{"type":"paragraph"}]}),
        1,
    )
    .await;
    let hidden = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    assert!(hidden["data"]["members"][0]["card"].is_null());
    app.api
        .put(&path)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"name":"Lo sợ","note":"Đổi ghi chú","card_ids":[card],"revision":1}))
        .await
        .assert_status_ok();
    save(&app, session, &note, 2).await;
    let restored = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    assert_eq!(restored["data"]["members"][0]["card"]["word"], "afraid");
    app.cleanup().await;
}
#[tokio::test]
async fn group_validation_rejects_duplicate_unavailable_unknown_and_oversized_fields_atomically() {
    let app = LearningApp::spawn().await;
    for body in [
        json!({"name":" ","note":"","card_ids":[]}),
        json!({"name":"x","note":"","card_ids":[1,1]}),
        json!({"name":"x","note":"","card_ids":[9999999]}),
        json!({"name":"x","note":"x".repeat(6001),"card_ids":[]}),
        json!({"name":"x","note":"","card_ids":[],"user_id":app.owner_id}),
    ] {
        app.api
            .post("/api/v1/word-groups")
            .add_header("authorization", auth(&app.owner_token))
            .json(&body)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        app.api
            .get("/api/v1/word-groups")
            .add_header("authorization", auth(&app.owner_token))
            .await
            .json::<Value>()["data"],
        json!([])
    );
    app.cleanup().await;
}

#[tokio::test]
async fn revoked_shared_note_never_leaks_through_saved_group_or_search() {
    let app = LearningApp::spawn().await;
    let session = lesson(&app, &app.owner_token).await;
    save(
        &app,
        session,
        &doc(vec![entry("afraid", "bí mật của nguồn", &[])]),
        0,
    )
    .await;
    let c = library(&app, &app.owner_token, "").await["data"][0]["id"]
        .as_i64()
        .unwrap();
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{session}/collaborators"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"email":"other@example.test"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let other = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let g = create(&app, &app.other_token, vec![c]).await;
    app.api
        .delete(&format!("/api/v1/sessions/{session}/collaborators/{other}"))
        .add_header("authorization", auth(&app.owner_token))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let path = format!("/api/v1/word-groups/{}", g["id"]);
    let hidden = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.other_token))
        .await
        .json::<Value>();
    assert!(hidden["data"]["members"][0]["card"].is_null());
    assert!(!hidden.to_string().contains("bí mật"));
    assert_eq!(
        app.api
            .get("/api/v1/word-groups/search?q=afraid")
            .add_header("authorization", auth(&app.other_token))
            .await
            .json::<Value>()["data"],
        json!([])
    );
    app.api
        .put(&path)
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"name":"Riêng tư","note":"","card_ids":[],"revision":1}))
        .await
        .assert_status_ok();
    app.cleanup().await;
}

#[tokio::test]
async fn parallel_group_saves_reject_stale_revision_without_overwriting_winner() {
    let app = LearningApp::spawn().await;
    let group = create(&app, &app.owner_token, vec![]).await;
    let path = format!("/api/v1/word-groups/{}", group["id"]);
    let a = app
        .api
        .put(&path)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"name":"a","note":"one","card_ids":[],"revision":1}));
    let b = app
        .api
        .put(&path)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"name":"b","note":"two","card_ids":[],"revision":1}));
    let (a, b) = tokio::join!(a, b);
    let statuses = [a.status_code(), b.status_code()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));
    let final_group = app
        .api
        .get(&path)
        .add_header("authorization", auth(&app.owner_token))
        .await
        .json::<Value>();
    let winner = if a.status_code() == StatusCode::OK {
        a.json::<Value>()
    } else {
        b.json::<Value>()
    };
    assert_eq!(final_group, winner);
    app.cleanup().await;
}
