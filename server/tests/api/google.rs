use super::helpers::{Identity, TestApp, REFRESH};

#[tokio::test]
async fn google_redirect_requests_identity_with_pkce_and_secure_browser_cookie() {
    let app = TestApp::spawn().await;
    let (params, _) = app.begin_google().await;
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["code_challenge_method"], "S256");
    for scope in ["openid", "email", "profile"] {
        assert!(params["scope"].contains(scope));
    }
}

#[tokio::test]
async fn verified_google_login_creates_user_and_reuses_same_identity() {
    let app = TestApp::spawn().await;
    let response = app.login(Identity::default()).await;
    response.assert_status_see_other();
    assert_eq!(response.header("location"), "https://frontend.example.test");
    let cookie = response.header("set-cookie").to_str().unwrap().to_string();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Lax")
    );
    app.api.post(REFRESH).await.assert_status_ok();
    app.login(Identity::default())
        .await
        .assert_status_see_other();
    assert_eq!(app.user_count().await, 1);
    let requests = app.google.received_requests().await.unwrap();
    assert!(String::from_utf8_lossy(&requests[0].body).contains("code_verifier="));
}

#[tokio::test]
async fn callback_rejects_missing_browser_session_wrong_state_and_replayed_attempt() {
    let mut app = TestApp::spawn().await;
    let (params, cookie) = app.begin_google().await;
    app.mock_token(&params["nonce"], Identity::default()).await;
    app.api.clear_cookies();
    app.callback(&params["state"])
        .await
        .assert_status_see_other();
    app.api.post(REFRESH).await.assert_status_unauthorized();
    let bad = app
        .api
        .get("/api/v1/auth/google/callback?state=wrong&code=test-code")
        .add_header("cookie", &cookie)
        .await;
    assert!(bad
        .header("location")
        .to_str()
        .unwrap()
        .contains("google_login_failed"));
    app.login(Identity::default()).await;
    let replay = app.callback(&params["state"]).await;
    assert!(replay
        .header("location")
        .to_str()
        .unwrap()
        .contains("google_login_failed"));
}

#[tokio::test]
async fn invalid_google_identity_does_not_create_user_or_authenticated_session() {
    let cases = [
        Identity {
            wrong_nonce: true,
            ..Default::default()
        },
        Identity {
            wrong_audience: true,
            ..Default::default()
        },
        Identity {
            expired: true,
            ..Default::default()
        },
        Identity {
            unverified: true,
            ..Default::default()
        },
        Identity {
            forged: true,
            ..Default::default()
        },
    ];
    for identity in cases {
        let app = TestApp::spawn().await;
        let response = app.login(identity).await;
        assert!(response
            .header("location")
            .to_str()
            .unwrap()
            .contains("google_login_failed"));
        app.api.post(REFRESH).await.assert_status_unauthorized();
        assert_eq!(app.user_count().await, 0);
    }
}

#[tokio::test]
async fn google_does_not_link_different_subjects_by_matching_email() {
    let mut app = TestApp::spawn().await;
    app.login(Identity::default()).await;
    app.api.clear_cookies();
    let other = app
        .login(Identity {
            subject: Some("another-google-user".into()),
            ..Default::default()
        })
        .await;
    assert!(other
        .header("location")
        .to_str()
        .unwrap()
        .contains("google_login_failed"));
    app.api.post(REFRESH).await.assert_status_unauthorized();
    assert_eq!(app.user_count().await, 1);
}
