use super::helpers::{Identity, TestApp, ME, REFRESH};

#[tokio::test]
async fn me_requires_bearer_token_and_returns_only_public_user_fields() {
    let app = TestApp::spawn().await;
    app.api.get(ME).await.assert_status_unauthorized();
    app.login(Identity::default()).await;
    // A valid session cookie alone cannot authorize business API requests.
    app.api.get(ME).await.assert_status_unauthorized();
    let token = app.access_token().await;
    let response = app
        .api
        .get(ME)
        .add_header("authorization", format!("Bearer {token}"))
        .await;
    response.assert_status_ok();
    let data = response.json::<serde_json::Value>();
    assert_eq!(data["data"]["email"], "user@example.test");
    assert_eq!(data["data"].as_object().unwrap().len(), 4);
    assert_eq!(response.header("cache-control"), "no-store");
    // Bearer authentication works independently of the session store.
    app.expire_sessions().await;
    app.api
        .get(ME)
        .add_header("authorization", format!("Bearer {token}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn saved_session_restores_refresh_in_recreated_application_and_expired_session_is_rejected() {
    let app = TestApp::spawn().await;
    let login = app.login(Identity::default()).await;
    let cookie = login
        .header("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let restarted = app.restart();
    restarted
        .post(REFRESH)
        .add_header("cookie", &cookie)
        .await
        .assert_status_ok();
    app.expire_sessions().await;
    app.restart()
        .post(REFRESH)
        .add_header("cookie", &cookie)
        .await
        .assert_status_unauthorized();
}
