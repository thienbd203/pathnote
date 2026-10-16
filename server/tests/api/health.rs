use super::helpers::TestApp;

#[tokio::test]
async fn health_reports_available_database() {
    let app = TestApp::spawn().await;
    let response = app.api.get("/health").await;
    response.assert_status_ok();
    assert_eq!(
        response.json::<serde_json::Value>()["data"]["database"],
        "ok"
    );
}

#[tokio::test]
async fn health_identifies_the_running_build() {
    let app = TestApp::spawn().await;
    let response = app.api.get("/health").await;
    response.assert_status_ok();
    assert_eq!(
        response.json::<serde_json::Value>()["data"]["version"],
        option_env!("PATHNOTE_BUILD_SHA").unwrap_or("development")
    );
}

#[tokio::test]
async fn request_ids_are_generated_and_propagated() {
    let app = TestApp::spawn().await;
    let response = app.api.get("/health").await;
    assert!(uuid::Uuid::parse_str(response.header("x-request-id").to_str().unwrap()).is_ok());
    let response = app
        .api
        .get("/health")
        .add_header("x-request-id", "test-request")
        .await;
    assert_eq!(response.header("x-request-id"), "test-request");
}

#[tokio::test]
async fn fallback_errors_keep_json_and_browser_security_headers() {
    let app = TestApp::spawn().await;
    let missing = app.api.get("/api/v1/does-not-exist").await;
    missing.assert_status_not_found();
    assert_eq!(missing.header("cache-control"), "no-store");
    assert_eq!(missing.header("referrer-policy"), "no-referrer");
    assert!(missing.json::<serde_json::Value>()["message"].is_string());
    let method = app.api.post("/health").await;
    method.assert_status(axum::http::StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(method.header("cache-control"), "no-store");
    assert_eq!(method.header("referrer-policy"), "no-referrer");
    assert!(method.json::<serde_json::Value>()["message"].is_string());
}
