use super::{learning_helpers::LearningApp, vocabulary::lesson};
use axum::http::StatusCode;
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, method, path},
    Mock, MockServer, ResponseTemplate,
};
fn auth(token: &str) -> String {
    format!("Bearer {token}")
}
#[tokio::test]
async fn enrichment_authorizes_before_calling_upstream_and_validates_success_and_partial_batch() {
    let mock = MockServer::start().await;
    let app = LearningApp::spawn_services(&mock.uri()).await;
    let id = lesson(&app, &app.owner_token).await;
    Mock::given(method("POST"))
        .and(path("/ipa"))
        .and(body_json(json!({"text":"teacher","locale":"en-GB"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"ipa":"/tiːtʃə/","locale":"en-GB"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    let route = format!("/api/v1/sessions/{id}/pronunciation");
    app.api
        .post(&route)
        .add_header("authorization", auth(&app.other_token))
        .json(&json!({"text":"teacher","locale":"en-GB"}))
        .await
        .assert_status_not_found();
    app.api
        .post(&route)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"teacher","locale":"vi"}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let single = app
        .api
        .post(&route)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":" teacher ","locale":"en-GB"}))
        .await;
    single.assert_status_ok();
    assert_eq!(single.json::<Value>()["data"]["ipa"], "/tiːtʃə/");
    Mock::given(method("POST")).and(path("/ipa/batch")).and(body_json(json!({"texts":["teacher","chef"],"locale":"en-US"}))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"locale":"en-US","results":[{"text":"teacher","ipa":"/teacher/"},{"text":"chef","error":"generation_failed"}]}))).expect(1).mount(&mock).await;
    let batch = app
        .api
        .post(&format!("{route}/batch"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"texts":["teacher","chef","teacher"],"locale":"en-US"}))
        .await;
    batch.assert_status_ok();
    let results = batch.json::<Value>()["data"]["results"].clone();
    assert_eq!(results.as_array().unwrap().len(), 3);
    assert_eq!(results[0], results[2]);
    assert!(results[1].get("error").is_some());
    Mock::given(method("POST"))
        .and(path("/translate"))
        .and(body_json(
            json!({"q":"teacher","source":"en","target":"vi","format":"text"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"translatedText":"giáo viên"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    let translation = app
        .api
        .post(&format!("/api/v1/sessions/{id}/translation"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"teacher"}))
        .await;
    translation.assert_status_ok();
    assert_eq!(translation.json::<Value>()["data"]["meaning"], "giáo viên");
    for request in mock.received_requests().await.unwrap() {
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("cookie"));
    }
    app.cleanup().await;
}
#[tokio::test]
async fn enrichment_returns_503_for_wrong_response_or_failure_and_rejects_oversized_batches() {
    let mock = MockServer::start().await;
    let app = LearningApp::spawn_services(&mock.uri()).await;
    let id = lesson(&app, &app.owner_token).await;
    let route = format!("/api/v1/sessions/{id}/pronunciation");
    Mock::given(path("/ipa"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"ipa":"/hello/","locale":"en-GB"})),
        )
        .mount(&mock)
        .await;
    app.api
        .post(&route)
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"hello","locale":"en-US"}))
        .await
        .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    Mock::given(path("/translate"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    app.api
        .post(&format!("/api/v1/sessions/{id}/translation"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"teacher"}))
        .await
        .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    app.api
        .post(&format!("{route}/batch"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"texts":vec!["word";51],"locale":"en-US"}))
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    Mock::given(path("/ipa/batch"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"locale":"en-US","results":[{"text":"wrong","ipa":"/hi/"}]})),
        )
        .mount(&mock)
        .await;
    app.api
        .post(&format!("{route}/batch"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"texts":["hello"],"locale":"en-US"}))
        .await
        .assert_status(StatusCode::SERVICE_UNAVAILABLE);
    app.cleanup().await;
}

#[tokio::test]
#[ignore = "Requires local eSpeak container on 8020 and LibreTranslate on 8022"]
async fn enrichment_with_real_local_services() {
    let app =
        LearningApp::spawn_service_urls("http://127.0.0.1:8020", "http://127.0.0.1:8022").await;
    let id = lesson(&app, &app.owner_token).await;
    let single = app
        .api
        .post(&format!("/api/v1/sessions/{id}/pronunciation"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"teacher","locale":"en-US"}))
        .await;
    single.assert_status_ok();
    assert!(!single.json::<Value>()["data"]["ipa"]
        .as_str()
        .unwrap()
        .is_empty());
    let batch = app
        .api
        .post(&format!("/api/v1/sessions/{id}/pronunciation/batch"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"texts":["teacher","chef","teacher"],"locale":"en-GB"}))
        .await;
    batch.assert_status_ok();
    let rows = batch.json::<Value>()["data"]["results"].clone();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    assert_eq!(rows[0], rows[2]);
    let translation = app
        .api
        .post(&format!("/api/v1/sessions/{id}/translation"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&json!({"text":"teacher"}))
        .await;
    translation.assert_status_ok();
    assert!(!translation.json::<Value>()["data"]["meaning"]
        .as_str()
        .unwrap()
        .is_empty());
    app.cleanup().await;
}

#[tokio::test]
async fn batch_size_limit_includes_the_json_envelope() {
    let app = LearningApp::spawn().await;
    let id = lesson(&app, &app.owner_token).await;
    let mut texts = vec!["é".repeat(1000); 32];
    texts.push("é".repeat(710));
    let input = json!({"texts": texts, "locale": "en-US"});
    assert!(serde_json::to_vec(&input["texts"]).unwrap().len() <= 65536);
    assert!(serde_json::to_vec(&input).unwrap().len() > 65536);
    let response = app
        .api
        .post(&format!("/api/v1/sessions/{id}/pronunciation/batch"))
        .add_header("authorization", auth(&app.owner_token))
        .json(&input)
        .await;
    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.json::<Value>()["errors"]["texts"].is_array());
    app.cleanup().await;
}
