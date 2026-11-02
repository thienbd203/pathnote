use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
#[tokio::test]
async fn retired_ipa_library_endpoints_are_not_exposed() {
    let app = LearningApp::spawn().await;
    let token = format!("Bearer {}", app.owner_token);
    for path in ["/api/v1/ipa-vocabulary", "/api/v1/ipa-pairs"] {
        app.api
            .get(path)
            .add_header("authorization", &token)
            .await
            .assert_status(StatusCode::NOT_FOUND);
        for action in ["import", "duplicates"] {
            app.api
                .post(&format!("{path}/{action}"))
                .add_header("authorization", &token)
                .json(&serde_json::json!({}))
                .await
                .assert_status(StatusCode::NOT_FOUND);
        }
    }
    app.cleanup().await;
}
