use axum_test::{TestResponse, TestServer};
use chrono::{Duration, Utc};
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey,
        CoreSubjectIdentifierType,
    },
    AccessToken, Audience, AuthUrl, EmptyAdditionalClaims, EmptyAdditionalProviderMetadata,
    EndUserEmail, IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl, Nonce, PrivateSigningKey,
    ResponseTypes, StandardClaims, SubjectIdentifier, TokenUrl,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema};
use serde_json::{json, Value};
use server::{app::build_app, config::Config, entities::users, state::AppState, GoogleAuth};
use std::{collections::HashMap, sync::Arc};
use tower_sessions_sqlx_store::SqliteStore;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

pub const ME: &str = "/api/v1/auth/me";
pub const REFRESH: &str = "/api/v1/auth/refresh";
pub const LOGOUT: &str = "/api/v1/auth/logout";
pub const GOOGLE: &str = "/api/v1/auth/google";

#[derive(Default)]
pub struct Identity {
    pub subject: Option<String>,
    pub email: Option<String>,
    pub wrong_nonce: bool,
    pub wrong_audience: bool,
    pub expired: bool,
    pub unverified: bool,
    pub forged: bool,
}

pub struct TestApp {
    pub api: TestServer,
    pub db: Arc<DatabaseConnection>,
    pub google: MockServer,
    config: Arc<Config>,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let db = Arc::new(Database::connect("sqlite::memory:").await.unwrap());
        let schema = Schema::new(db.get_database_backend());
        for table in [schema.create_table_from_entity(users::Entity)] {
            db.execute(db.get_database_backend().build(&table))
                .await
                .unwrap();
        }
        SqliteStore::new(db.get_sqlite_connection_pool().clone())
            .with_table_name("tower_sessions")
            .unwrap()
            .migrate()
            .await
            .unwrap();
        let google = MockServer::start().await;
        let config = Arc::new(Config {
            google_client_id: "our-client".into(),
            google_client_secret: "test-secret".into(),
            google_redirect_url: "http://localhost:3000/api/v1/auth/google/callback".into(),
            cookie_secure: true,
            frontend_url: "https://frontend.example.test".into(),
            bind_addr: "127.0.0.1:3000".into(),
            database_url: "sqlite::memory:".into(),
            jwt_private_key_path: "unused-test-path".into(),
            jwt_issuer: "https://api.example.test".into(),
            jwt_audience: "pathnote-api".into(),
            jwt_key_id: "test-access-key".into(),
        });
        let api = Self::server(db.clone(), config.clone(), &google);
        Self {
            api,
            db,
            google,
            config,
        }
    }
    fn server(db: Arc<DatabaseConnection>, config: Arc<Config>, google: &MockServer) -> TestServer {
        let google_client = GoogleAuth::new(config.clone())
            .unwrap()
            .with_provider_metadata(metadata(&google.uri()))
            .unwrap();
        let access_tokens = server::access_token::AccessTokens::from_pem(
            include_str!("../fixtures/test-only-rsa.pem"),
            config.jwt_issuer.clone(),
            config.jwt_audience.clone(),
            config.jwt_key_id.clone(),
        )
        .unwrap();
        let state = AppState {
            access_tokens: Arc::new(access_tokens),
            db: db.clone(),
            google: Arc::new(google_client),
            config: config.clone(),
        };
        let mut api = TestServer::builder().save_cookies().build(build_app(
            state,
            &config,
            SqliteStore::new(db.get_sqlite_connection_pool().clone())
                .with_table_name("tower_sessions")
                .unwrap(),
        ));
        api.add_header("origin", config.frontend_url.as_str());
        api
    }
    pub fn restart(&self) -> TestServer {
        Self::server(self.db.clone(), self.config.clone(), &self.google)
    }
    pub async fn begin_google(&self) -> (HashMap<String, String>, String) {
        let response = self.api.get(GOOGLE).await;
        response.assert_status_see_other();
        let url = reqwest::Url::parse(response.header("location").to_str().unwrap()).unwrap();
        let cookie = response
            .header("set-cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        (url.query_pairs().into_owned().collect(), cookie)
    }
    pub async fn callback(&self, state: &str) -> TestResponse {
        self.api
            .get(&format!(
                "/api/v1/auth/google/callback?state={state}&code=test-code"
            ))
            .await
    }
    pub async fn login(&self, identity: Identity) -> TestResponse {
        let (params, _) = self.begin_google().await;
        self.mock_token(&params["nonce"], identity).await;
        self.callback(&params["state"]).await
    }
    pub async fn access_token(&self) -> String {
        let response = self.api.post(REFRESH).await;
        response.assert_status_ok();
        response.json::<Value>()["data"]["access_token"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    pub async fn mock_token(&self, nonce: &str, identity: Identity) {
        self.google.reset().await;
        let response = token_response(nonce, identity);
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&self.google)
            .await;
    }
    pub async fn expire_sessions(&self) {
        tower_sessions_sqlx_store::sqlx::query("UPDATE tower_sessions SET expiry_date = ?")
            .bind(time::OffsetDateTime::now_utc() - time::Duration::days(1))
            .execute(self.db.get_sqlite_connection_pool())
            .await
            .unwrap();
    }
    pub async fn user_count(&self) -> u64 {
        use sea_orm::PaginatorTrait;
        users::Entity::find().count(self.db.as_ref()).await.unwrap()
    }
}
fn signing_key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(
        include_str!("../fixtures/test-only-rsa.pem"),
        Some(JsonWebKeyId::new("test-key".into())),
    )
    .unwrap()
}
fn metadata(base: &str) -> CoreProviderMetadata {
    CoreProviderMetadata::new(
        IssuerUrl::new("https://accounts.google.com".into()).unwrap(),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".into()).unwrap(),
        JsonWebKeySetUrl::new(format!("{base}/jwks")).unwrap(),
        vec![ResponseTypes::new(vec![CoreResponseType::Code])],
        vec![CoreSubjectIdentifierType::Public],
        vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
        EmptyAdditionalProviderMetadata {},
    )
    .set_token_endpoint(Some(TokenUrl::new(format!("{base}/token")).unwrap()))
    .set_jwks(CoreJsonWebKeySet::new(vec![
        signing_key().as_verification_key()
    ]))
}
fn token_response(nonce: &str, identity: Identity) -> Value {
    let access = AccessToken::new("test-access".into());
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new("https://accounts.google.com".into()).unwrap(),
        vec![Audience::new(
            if identity.wrong_audience {
                "other-client"
            } else {
                "our-client"
            }
            .into(),
        )],
        Utc::now() + Duration::seconds(if identity.expired { -300 } else { 300 }),
        Utc::now(),
        StandardClaims::new(SubjectIdentifier::new(
            identity.subject.unwrap_or_else(|| "google-user".into()),
        ))
        .set_email(Some(EndUserEmail::new(
            identity.email.unwrap_or_else(|| "USER@example.test".into()),
        )))
        .set_email_verified(Some(!identity.unverified)),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(
        if identity.wrong_nonce {
            "wrong-nonce"
        } else {
            nonce
        }
        .into(),
    )));
    let mut token = CoreIdToken::new(
        claims,
        &signing_key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&access),
        None,
    )
    .unwrap()
    .to_string();
    if identity.forged {
        let mut parts: Vec<String> = token.split('.').map(str::to_owned).collect();
        let replacement = if parts[2].starts_with('A') { "B" } else { "A" };
        parts[2].replace_range(..1, replacement);
        token = parts.join(".");
    }
    json!({"access_token":"test-access", "token_type":"Bearer", "expires_in":300, "id_token":token})
}
