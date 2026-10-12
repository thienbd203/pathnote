use super::helpers::{Identity, TestApp, LOGOUT, REFRESH};

#[tokio::test]
async fn logout_revokes_session_and_old_cookie_cannot_be_reused() {
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
    app.api.post(LOGOUT).await.assert_status_no_content();
    app.api.post(REFRESH).await.assert_status_unauthorized();
    app.restart()
        .post(REFRESH)
        .add_header("cookie", &cookie)
        .await
        .assert_status_unauthorized();
}

#[tokio::test]
async fn logout_rejects_foreign_origin_or_missing_origin() {
    let mut app = TestApp::spawn().await;
    app.api.clear_headers();
    app.api.post(LOGOUT).await.assert_status_forbidden();
    app.api
        .post(LOGOUT)
        .add_header("origin", "https://another-site.test")
        .await
        .assert_status_forbidden();
}
