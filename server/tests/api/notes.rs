use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use server::entities::{note_collaborators, notes};
fn token(s: &str) -> String {
    format!("Bearer {s}")
}
async fn lesson(app: &LearningApp) -> i64 {
    let r = app
        .api
        .post("/api/v1/places")
        .add_header("authorization", token(&app.owner_token))
        .json(&json!({"name":"Notes school"}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let id = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r=app.api.post(&format!("/api/v1/places/{id}/sessions")).add_header("authorization",token(&app.owner_token)).json(&json!({"title":"Lesson","starts_at":"2026-10-13T11:00:00Z","duration_minutes":60,"timezone":"Asia/Ho_Chi_Minh"})).await;
    r.assert_status(StatusCode::CREATED);
    r.json::<Value>()["data"]["id"].as_i64().unwrap()
}
fn doc() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","attrs":{"textAlign":null,"indent":0},"content":[{"type":"text","text":"Hello Jobs","marks":[{"type":"bold"}]}]}]})
}
fn save(content: Value, revision: i32) -> Value {
    json!({"content":content,"schema_version":4,"base_revision":revision})
}
#[tokio::test]
async fn notes_roundtrip_jsonb_and_revision_conflict_preserves_original() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/sessions/{id}/note");
    let auth = token(&app.owner_token);
    app.api.get(&path).await.assert_status_unauthorized();
    app.api
        .get(&path)
        .add_header("authorization", token(&app.other_token))
        .await
        .assert_status_not_found();
    let r = app.api.get(&path).add_header("authorization", &auth).await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["revision"], 0);
    let content = doc();
    let r = app
        .api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&save(content.clone(), 0))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["content"], content);
    assert_eq!(r.json::<Value>()["data"]["revision"], 1);
    let r = app
        .api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&save(
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            0,
        ))
        .await;
    r.assert_status(StatusCode::CONFLICT);
    assert_eq!(r.json::<Value>()["data"]["content"], content);
    let stored = notes::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.content, content);
    app.cleanup().await;
}
#[tokio::test]
async fn notes_typed_nodes_reject_wrong_attributes_shapes_links_and_limits() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/sessions/{id}/note");
    let auth = token(&app.owner_token);
    let cases = vec![
        json!({"type":"text","text":"not a block"}),
        json!({"type":"heading","attrs":{"level":"1"}}),
        json!({"type":"heading","attrs":{"level":4}}),
        json!({"type":"paragraph","attrs":{"onclick":"attack"}}),
        json!({"type":"paragraph","content":[{"type":"table"}]}),
        json!({"type":"taskItem","attrs":{"checked":1},"content":[{"type":"paragraph"}]}),
        json!({"type":"paragraph","content":[{"type":"text","text":"x","marks":[{"type":"link","attrs":{"href":"javascript:alert(1)"}}]}]}),
        json!({"type":"paragraph","content":[{"type":"text","text":"x","marks":[{"type":"bold"},{"type":"bold"}]}]}),
        json!({"type":"image","attrs":{"mediaId":1}}),
        json!({"type":"exercise","attrs":{"exerciseId":1}}),
        json!({"type":"table","content":[{"type":"tableRow","content":[{"type":"tableCell","attrs":{"rowspan":2},"content":[{"type":"paragraph"}]}]}]}),
        json!({"type":"paragraph","content":[{"type":"text","text":"x".repeat(262144)}]}),
    ];
    for node in cases {
        app.api
            .put(&path)
            .add_header("authorization", &auth)
            .json(&save(json!({"type":"doc","content":[node]}), 0))
            .await
            .assert_status_unprocessable_entity();
    }
    let many = json!({"type":"doc","content":vec![json!({"type":"paragraph"});10000]});
    app.api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&save(many, 0))
        .await
        .assert_status_unprocessable_entity();
    let mut deep = json!({"type":"paragraph"});
    for _ in 0..33 {
        deep = json!({"type":"blockquote","content":[deep]});
    }
    app.api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&save(json!({"type":"doc","content":[deep]}), 0))
        .await
        .assert_status_unprocessable_entity();
    let mut body = save(doc(), 0);
    body["title"] = json!("Can't rename through note");
    app.api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&body)
        .await
        .assert_status_unprocessable_entity();
    assert!(notes::Entity::find_by_id(id)
        .one(app.db.as_ref())
        .await
        .unwrap()
        .is_none());
    app.cleanup().await;
}
#[tokio::test]
async fn notes_accept_rich_editor_blocks_and_preserve_json_without_normalizing() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let auth = token(&app.owner_token);
    let content = json!({"type":"doc","content":[
    {"type":"heading","attrs":{"level":2,"textAlign":"center","indent":1},"content":[{"type":"text","text":"Jobs"}]},
    {"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":false},"content":[{"type":"paragraph"}]}]},
    {"type":"orderedList","attrs":{"start":2,"type":"a"},"content":[{"type":"listItem","content":[{"type":"paragraph"}]}]},
    {"type":"codeBlock","attrs":{"language":"rust"},"content":[{"type":"text","text":"fn main() {}"}]},
    {"type":"table","content":[{"type":"tableRow","content":[{"type":"tableCell","attrs":{"colspan":2,"rowspan":1,"colwidth":[80,100],"align":null},"content":[{"type":"paragraph"}]}]}]},
    {"type":"vocabulary","attrs":{"locale":"en-US"},"content":[{"type":"vocabularyEntry","attrs":{"id":"00000000-0000-0000-0000-000000000001","word":"chef","ipa":"/ʃef/","topics":["Jobs"]},"content":[{"type":"vocabularySense","attrs":{"id":"00000000-0000-0000-0000-000000000002","partOfSpeech":"noun","meaning":"đầu bếp"},"content":[{"type":"vocabularyExample","attrs":{"id":"00000000-0000-0000-0000-000000000003","text":"I am a chef.","highlights":["chef"]}}]}]}]},
    {"type":"paragraph","content":[{"type":"text","text":"Read","marks":[{"type":"link","attrs":{"href":"https://example.com","target":"_blank","rel":"noopener noreferrer nofollow","class":null}},{"type":"textStyle","attrs":{"color":"#ff0000","backgroundColor":null}}]}]}
    ]});
    let r = app
        .api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", &auth)
        .json(&save(content.clone(), 0))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["content"], content);
    app.cleanup().await;
}
#[tokio::test]
async fn sharing_is_idempotent_and_collaborators_edit_only_notes_until_revoked() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let owner = token(&app.owner_token);
    let other = token(&app.other_token);
    let path = format!("/api/v1/sessions/{id}/collaborators");
    for _ in 0..2 {
        app.api
            .post(&path)
            .add_header("authorization", &owner)
            .json(&json!({"email":" OTHER@example.test "}))
            .await
            .assert_status(StatusCode::CREATED);
    }
    let members = app.api.get(&path).add_header("authorization", &owner).await;
    members.assert_status_ok();
    let list = members.json::<Value>()["data"].as_array().unwrap().clone();
    assert_eq!(list.len(), 1);
    let user = list[0]["id"].as_i64().unwrap();
    app.api
        .get(&path)
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .post(&path)
        .add_header("authorization", &other)
        .json(&json!({"email":"owner@example.test"}))
        .await
        .assert_status_not_found();
    for email in ["owner@example.test", "missing@example.test"] {
        app.api
            .post(&path)
            .add_header("authorization", &owner)
            .json(&json!({"email":email}))
            .await
            .assert_status_unprocessable_entity();
    }
    app.api
        .get(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", &other)
        .json(&save(doc(), 0))
        .await
        .assert_status_ok();
    let shared = app
        .api
        .get("/api/v1/shared-notes")
        .add_header("authorization", &other)
        .await;
    shared.assert_status_ok();
    assert_eq!(shared.json::<Value>()["meta"]["total"], 1);
    app.api
        .get(&format!("/api/v1/shared-notes/{id}"))
        .add_header("authorization", &other)
        .await
        .assert_status_ok();
    app.api
        .delete(&format!("{path}/{user}"))
        .add_header("authorization", &owner)
        .await
        .assert_status_no_content();
    app.api
        .get(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", &other)
        .await
        .assert_status_not_found();
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", &other)
        .json(&save(doc(), 1))
        .await
        .assert_status_not_found();
    assert!(note_collaborators::Entity::find()
        .filter(note_collaborators::Column::SessionId.eq(id))
        .all(app.db.as_ref())
        .await
        .unwrap()
        .is_empty());
    app.cleanup().await;
}
#[tokio::test]
async fn concurrent_note_saves_have_one_winner_and_deleted_parents_hide_shared_notes() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let owner = token(&app.owner_token);
    let path = format!("/api/v1/sessions/{id}/note");
    let body = save(doc(), 0);
    let (a, b) = tokio::join!(
        app.api
            .put(&path)
            .add_header("authorization", &owner)
            .json(&body),
        app.api
            .put(&path)
            .add_header("authorization", &owner)
            .json(&body)
    );
    let mut statuses = [a.status_code().as_u16(), b.status_code().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    app.api
        .post(&format!("/api/v1/sessions/{id}/collaborators"))
        .add_header("authorization", &owner)
        .json(&json!({"email":"other@example.test"}))
        .await
        .assert_status(StatusCode::CREATED);
    app.api
        .delete(&format!("/api/v1/sessions/{id}"))
        .add_header("authorization", &owner)
        .await
        .assert_status_no_content();
    app.api
        .get(&path)
        .add_header("authorization", &owner)
        .await
        .assert_status_not_found();
    let r = app
        .api
        .get("/api/v1/shared-notes")
        .add_header("authorization", token(&app.other_token))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["meta"]["total"], 0);
    app.cleanup().await;
}

#[tokio::test]
async fn shared_note_search_filters_titles_and_preserves_membership_scope() {
    let app = LearningApp::spawn().await;
    let shared = lesson(&app).await;
    let _private = lesson(&app).await;
    let owner = token(&app.owner_token);
    let other = token(&app.other_token);
    let path = format!("/api/v1/sessions/{shared}/collaborators");
    app.api
        .post(&path)
        .add_header("authorization", &owner)
        .json(&json!({"email":"other@example.test"}))
        .await
        .assert_status(StatusCode::CREATED);
    for (q, expected) in [("LESS", 1), ("missing", 0), ("%", 0), ("_", 0)] {
        let response = app
            .api
            .get("/api/v1/shared-notes")
            .add_header("authorization", &other)
            .add_query_param("q", q)
            .await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["meta"]["total"], expected);
        if expected == 1 {
            assert_eq!(response.json::<Value>()["data"][0]["id"], shared);
        }
    }
    let response = app.api.get(&path).add_header("authorization", &owner).await;
    let member = response.json::<Value>()["data"][0]["id"].as_i64().unwrap();
    app.api
        .delete(&format!("{path}/{member}"))
        .add_header("authorization", &owner)
        .await
        .assert_status_no_content();
    let response = app
        .api
        .get("/api/v1/shared-notes?q=Lesson")
        .add_header("authorization", &other)
        .await;
    response.assert_status_ok();
    assert_eq!(response.json::<Value>()["meta"]["total"], 0);
    app.cleanup().await;
}

#[tokio::test]
async fn vocabulary_dual_accents_roundtrip_rest_and_collaboration_snapshots() {
    for realtime in [false, true] {
        let app = LearningApp::spawn_with_collaboration(realtime).await;
        let id = lesson(&app).await;
        let mut content = json!({"type":"doc","content":[{"type":"vocabulary","attrs":{"locale":"en-US"},"content":[{"type":"vocabularyEntry","attrs":{"id":"00000000-0000-0000-0000-000000000001","word":"history","ipa":"/US/","ipaUK":"/UK/","ipaUS":"/US/","topics":[]},"content":[{"type":"vocabularySense","attrs":{"id":"00000000-0000-0000-0000-000000000002","partOfSpeech":"noun","meaning":"lịch sử"},"content":[{"type":"vocabularyExample","attrs":{"id":"00000000-0000-0000-0000-000000000003","text":"I study history.","highlights":[],"ipa":"/US sentence/","ipaUK":"/UK sentence/","ipaUS":"/US sentence/"}}]}]}]}]});
        let public = format!("/api/v1/sessions/{id}/note");
        let internal = format!("/api/v1/internal/notes/{id}");
        let path = if realtime { &internal } else { &public };
        let header = if realtime {
            "x-collaboration-secret"
        } else {
            "authorization"
        };
        let auth = if realtime {
            "test-collaboration-secret-32-characters".to_owned()
        } else {
            token(&app.owner_token)
        };
        let payload = if realtime {
            json!({"content":content,"schema_version":4,"state":"AAA=","base_revision":0})
        } else {
            save(content.clone(), 0)
        };
        app.api
            .put(path)
            .add_header(header, &auth)
            .json(&payload)
            .await
            .assert_status_ok();
        let response = app
            .api
            .get(&public)
            .add_header("authorization", token(&app.owner_token))
            .await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["data"]["content"], content);
        content["content"][0]["content"][0]["attrs"]["ipaUK"] = json!("x".repeat(6001));
        let invalid = if realtime {
            json!({"content":content,"schema_version":4,"state":"AQI=","base_revision":1})
        } else {
            save(content, 1)
        };
        app.api
            .put(path)
            .add_header(header, &auth)
            .json(&invalid)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        app.cleanup().await;
    }
}

#[tokio::test]
async fn old_schema_save_cannot_erase_dual_accent_ipa() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let path = format!("/api/v1/sessions/{id}/note");
    let auth = token(&app.owner_token);
    let content = json!({"type":"doc","content":[{"type":"vocabulary","attrs":{"locale":"en-US"},"content":[{"type":"vocabularyEntry","attrs":{"id":"00000000-0000-0000-0000-000000000001","word":"history","ipa":"/US/","ipaUK":"/UK/","ipaUS":"/US/","topics":[]},"content":[{"type":"vocabularySense","attrs":{"id":"00000000-0000-0000-0000-000000000002","partOfSpeech":"noun","meaning":"lịch sử"},"content":[{"type":"vocabularyExample","attrs":{"id":"00000000-0000-0000-0000-000000000003","text":"I study history.","highlights":[],"ipaUK":"/UK sentence/","ipaUS":"/US sentence/"}}]}]}]}]});
    app.api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&save(content.clone(), 0))
        .await
        .assert_status_ok();
    // Already-deployed dual-IPA notes still have schema 3. A guard based only
    // on the stored document version would miss this real rollout case.
    use sea_orm::sea_query::Expr;
    notes::Entity::update_many()
        .col_expr(notes::Column::SchemaVersion, Expr::value(3))
        .filter(notes::Column::SessionId.eq(id))
        .exec(app.db.as_ref())
        .await
        .unwrap();
    let mut stripped = content.clone();
    fn strip(value: &mut Value) {
        if let Some(attrs) = value.get_mut("attrs").and_then(Value::as_object_mut) {
            attrs.remove("ipaUK");
            attrs.remove("ipaUS");
        }
        if let Some(children) = value.get_mut("content").and_then(Value::as_array_mut) {
            for child in children {
                strip(child);
            }
        }
    }
    strip(&mut stripped);
    let response = app
        .api
        .put(&path)
        .add_header("authorization", &auth)
        .json(&json!({"content":stripped,"schema_version":3,"base_revision":1}))
        .await;
    response.assert_status(StatusCode::CONFLICT);
    assert_eq!(response.json::<Value>()["code"], "client_outdated");
    let persisted = app
        .api
        .get(&path)
        .add_header("authorization", &auth)
        .await
        .json::<Value>();
    assert_eq!(persisted["data"]["content"], content);
    assert_eq!(persisted["data"]["revision"], 1);
    app.cleanup().await;
}

#[tokio::test]
async fn rest_and_bridge_persist_new_and_legacy_revision_zero_notes_with_identical_timestamps() {
    use sea_orm::{ActiveModelTrait, Set};
    use server::entities::sessions;
    for bridge in [false, true] {
        let app = LearningApp::spawn_with_collaboration(bridge).await;
        for legacy in [false, true] {
            let id = lesson(&app).await;
            if legacy {
                notes::ActiveModel {
                    session_id: Set(id),
                    content: Set(doc()),
                    yjs_state: Set(None),
                    schema_version: Set(3),
                    revision: Set(0),
                    saved_at: Set(chrono::Utc::now().fixed_offset()),
                }
                .insert(app.db.as_ref())
                .await
                .unwrap();
            }
            for revision in 0..2 {
                let mut content = doc();
                content["content"][0]["content"][0]["text"] = json!(format!("Revision {revision}"));
                let response = if bridge {
                    app.api.put(&format!("/api/v1/internal/notes/{id}"))
                        .add_header("x-collaboration-secret", "test-collaboration-secret-32-characters")
                        .json(&json!({"content":content,"schema_version":4,"base_revision":revision,"state":if revision == 0 {"AAA="} else {"AQI="}})).await
                } else {
                    app.api
                        .put(&format!("/api/v1/sessions/{id}/note"))
                        .add_header("authorization", token(&app.owner_token))
                        .json(&save(content.clone(), revision))
                        .await
                };
                response.assert_status_ok();
                let note = notes::Entity::find_by_id(id)
                    .one(app.db.as_ref())
                    .await
                    .unwrap()
                    .unwrap();
                let session = sessions::Entity::find_by_id(id)
                    .one(app.db.as_ref())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(note.saved_at, session.updated_at);
                assert_eq!(note.revision, revision + 1);
                assert_eq!(note.content, content);
                assert_eq!(note.yjs_state.is_some(), bridge);
            }
        }
        app.cleanup().await;
    }
}

#[tokio::test]
async fn persisted_realtime_note_with_disabled_bridge_returns_conflict_and_preserves_data() {
    use sea_orm::{ActiveModelTrait, Set};
    let app = LearningApp::spawn().await;
    let id = lesson(&app).await;
    let saved = notes::ActiveModel {
        session_id: Set(id),
        content: Set(doc()),
        yjs_state: Set(Some(vec![1, 2, 3])),
        schema_version: Set(4),
        revision: Set(1),
        saved_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(app.db.as_ref())
    .await
    .unwrap();
    app.api
        .get(&format!("/api/v1/sessions/{id}/collaboration"))
        .add_header("authorization", token(&app.owner_token))
        .await
        .assert_status(StatusCode::CONFLICT);
    let changed = json!({"type":"doc","content":[{"type":"paragraph"}]});
    app.api
        .put(&format!("/api/v1/sessions/{id}/note"))
        .add_header("authorization", token(&app.owner_token))
        .json(&save(changed, 1))
        .await
        .assert_status(StatusCode::CONFLICT);
    assert_eq!(
        notes::Entity::find_by_id(id)
            .one(app.db.as_ref())
            .await
            .unwrap()
            .unwrap(),
        saved
    );
    app.cleanup().await;
}
