use super::helpers::{Identity, TestApp, LOGOUT, ME, REFRESH};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::{json, Value};

const JWKS: &str = "/api/v1/.well-known/jwks.json";

#[tokio::test]
async fn refresh_issues_fifteen_minute_token_verifiable_with_public_jwks() {
    let app = TestApp::spawn().await;
    app.api.post(REFRESH).await.assert_status_unauthorized();
    app.login(Identity::default()).await;
    let response = app.api.post(REFRESH).await;
    response.assert_status_ok();
    assert_eq!(response.header("cache-control"), "no-store");
    let body = response.json::<Value>();
    assert_eq!(body["data"]["expires_in"], 900);
    assert_eq!(body["data"]["token_type"], "Bearer");
    let keys = app.api.get(JWKS).await.json::<Value>();
    let key = &keys["keys"][0];
    assert_eq!(key["alg"], "RS256");
    assert_eq!(key["kid"], "test-access-key");
    for private_component in ["d", "p", "q", "dp", "dq", "qi"] {
        assert!(key.get(private_component).is_none());
    }
    let decoding = DecodingKey::from_jwk(&serde_json::from_value(key.clone()).unwrap()).unwrap();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://api.example.test"]);
    validation.set_audience(&["pathnote-api"]);
    let claims = decode::<Value>(
        body["data"]["access_token"].as_str().unwrap(),
        &decoding,
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(
        claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
        900
    );
    assert_eq!(
        claims["sub"],
        body["data"]["user"]["id"].as_i64().unwrap().to_string()
    );
    assert_eq!(claims["token_use"], "access");
}

#[tokio::test]
async fn refresh_requires_browser_origin_and_session_not_just_access_token() {
    let mut app = TestApp::spawn().await;
    app.login(Identity::default()).await;
    let token = app.access_token().await;
    app.api.clear_headers();
    app.api.post(REFRESH).await.assert_status_forbidden();
    app.api
        .post(REFRESH)
        .add_header("origin", "https://evil.example.test")
        .await
        .assert_status_forbidden();
    app.api.clear_cookies();
    app.api
        .post(REFRESH)
        .add_header("origin", "https://frontend.example.test")
        .add_header("authorization", format!("Bearer {token}"))
        .await
        .assert_status_unauthorized();
}

#[tokio::test]
async fn logout_blocks_refresh_but_already_issued_access_token_remains_valid_until_expiry() {
    let app = TestApp::spawn().await;
    app.login(Identity::default()).await;
    let token = app.access_token().await;
    app.api.post(LOGOUT).await.assert_status_no_content();
    app.api.post(REFRESH).await.assert_status_unauthorized();
    app.api
        .get(ME)
        .add_header("authorization", format!("Bearer {token}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn bearer_api_rejects_invalid_signature_expiry_issuer_audience_and_token_type() {
    let app = TestApp::spawn().await;
    app.login(Identity::default()).await;
    let user = app.api.post(REFRESH).await.json::<Value>()["data"]["user"]["id"]
        .as_i64()
        .unwrap();
    let now = jsonwebtoken::get_current_timestamp();
    let base = json!({"sub": user.to_string(), "iss": "https://api.example.test", "aud": "pathnote-api", "iat": now, "exp": now + 900, "token_use": "access"});
    let mut cases = Vec::new();
    for (field, value) in [
        ("iss", json!("https://other.example.test")),
        ("aud", json!("other-service")),
        ("token_use", json!("refresh")),
        ("exp", json!(now - 1)),
        ("exp", json!(now + 901)),
        ("iat", json!(now + 60)),
        ("sub", json!("0")),
    ] {
        let mut claims = base.clone();
        claims[field] = value;
        cases.push(claims);
    }
    let key = EncodingKey::from_rsa_pem(include_bytes!("../fixtures/test-only-rsa.pem")).unwrap();
    let mut header = Header::new(Algorithm::RS256);
    header.typ = Some("at+jwt".into());
    header.kid = Some("test-access-key".into());
    for claims in cases {
        let token = encode(&header, &claims, &key).unwrap();
        app.api
            .get(ME)
            .add_header("authorization", format!("Bearer {token}"))
            .await
            .assert_status_unauthorized();
    }
    for (typ, kid) in [("JWT", "test-access-key"), ("at+jwt", "unknown-key")] {
        header.typ = Some(typ.into());
        header.kid = Some(kid.into());
        let token = encode(&header, &base, &key).unwrap();
        app.api
            .get(ME)
            .add_header("authorization", format!("Bearer {token}"))
            .await
            .assert_status_unauthorized();
    }
    let token = app.access_token().await;
    let mut parts: Vec<String> = token.split('.').map(str::to_owned).collect();
    let replacement = if parts[2].starts_with('A') { "B" } else { "A" };
    parts[2].replace_range(..1, replacement);
    app.api
        .get(ME)
        .add_header("authorization", format!("Bearer {}", parts.join(".")))
        .await
        .assert_status_unauthorized();
    let mut wrong_algorithm = Header::new(Algorithm::HS256);
    wrong_algorithm.typ = Some("at+jwt".into());
    wrong_algorithm.kid = Some("test-access-key".into());
    let token = encode(
        &wrong_algorithm,
        &base,
        &EncodingKey::from_secret(b"wrong-algorithm"),
    )
    .unwrap();
    app.api
        .get(ME)
        .add_header("authorization", format!("Bearer {token}"))
        .await
        .assert_status_unauthorized();
}
