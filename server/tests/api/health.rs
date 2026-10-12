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
