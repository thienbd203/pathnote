use crate::{config::Config, error::AppError};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct GoogleAuth {
    config: Arc<Config>,
    http: reqwest::Client,
    metadata: RwLock<Option<(Instant, CoreProviderMetadata)>>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginAttempt {
    pub state: CsrfToken,
    pub nonce: String,
    pub verifier: String,
    pub created_at: i64,
}

pub struct GoogleCredentials {
    pub code: String,
    pub attempt: LoginAttempt,
}
pub struct GoogleProfile {
    pub subject: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

impl LoginAttempt {
    pub fn validate_state(&self, state: &str, now: i64) -> bool {
        (0..=600).contains(&(now - self.created_at))
            && self.state == CsrfToken::new(state.to_owned())
    }
}

impl GoogleAuth {
    pub fn new(config: Arc<Config>) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Self {
            config,
            http,
            metadata: RwLock::new(None),
        })
    }
    /// Uses already-discovered provider metadata (e.g. supplied by an offline test provider).
    /// Callers must obtain this metadata from a trusted source, never a browser request.
    pub fn with_provider_metadata(
        mut self,
        metadata: CoreProviderMetadata,
    ) -> Result<Self, AppError> {
        if metadata.issuer().as_str() != "https://accounts.google.com" {
            return Err(AppError::BadRequest("Google issuer không hợp lệ".into()));
        }
        self.metadata = RwLock::new(Some((Instant::now(), metadata)));
        Ok(self)
    }

    async fn client(&self) -> Result<Client, AppError> {
        const TTL: Duration = Duration::from_secs(300);
        let cached = self
            .metadata
            .read()
            .await
            .as_ref()
            .filter(|(at, _)| at.elapsed() <= TTL)
            .map(|(_, metadata)| metadata.clone());
        let metadata = if let Some(metadata) = cached {
            metadata
        } else {
            // Serialize refreshes, but recheck after waiting: a previous writer
            // may already have completed discovery while we queued for this lock.
            let mut cache = self.metadata.write().await;
            if cache.as_ref().is_none_or(|(at, _)| at.elapsed() > TTL) {
                let metadata = CoreProviderMetadata::discover_async(
                    IssuerUrl::new("https://accounts.google.com".into()).unwrap(),
                    &self.http,
                )
                .await
                .map_err(|_| AppError::Internal("Google discovery unavailable".into()))?;
                *cache = Some((Instant::now(), metadata));
            }
            cache.as_ref().expect("metadata was refreshed").1.clone()
        };
        Ok(CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(self.config.google_client_id.clone()),
            Some(ClientSecret::new(self.config.google_client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(self.config.google_redirect_url.clone())
                .map_err(|_| AppError::Internal("Invalid Google redirect".into()))?,
        ))
    }
    pub async fn authorization(&self) -> Result<(String, LoginAttempt), AppError> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        Ok((
            url.to_string(),
            LoginAttempt {
                state,
                nonce: nonce.secret().clone(),
                verifier: verifier.secret().clone(),
                created_at: chrono::Utc::now().timestamp(),
            },
        ))
    }
    pub async fn verify(&self, credentials: GoogleCredentials) -> Result<GoogleProfile, AppError> {
        let client = self.client().await?;
        let response = client
            .exchange_code(AuthorizationCode::new(credentials.code))
            .map_err(|_| AppError::Unauthenticated)?
            .set_pkce_verifier(PkceCodeVerifier::new(credentials.attempt.verifier))
            .request_async(&self.http)
            .await
            .map_err(|_| AppError::Unauthenticated)?;
        verify_response(&client, &response, Nonce::new(credentials.attempt.nonce))
    }
}

fn verify_response(
    client: &Client,
    response: &openidconnect::core::CoreTokenResponse,
    nonce: Nonce,
) -> Result<GoogleProfile, AppError> {
    let id_token = response.id_token().ok_or(AppError::Unauthenticated)?;
    let verifier = client.id_token_verifier();
    let claims = id_token
        .claims(&verifier, &nonce)
        .map_err(|_| AppError::Unauthenticated)?;
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            response.access_token(),
            id_token
                .signing_alg()
                .map_err(|_| AppError::Unauthenticated)?,
            id_token
                .signing_key(&verifier)
                .map_err(|_| AppError::Unauthenticated)?,
        )
        .map_err(|_| AppError::Unauthenticated)?;
        if actual != *expected {
            return Err(AppError::Unauthenticated);
        }
    }
    if claims.email_verified() != Some(true) {
        return Err(AppError::Unauthenticated);
    }
    let email = claims
        .email()
        .ok_or(AppError::Unauthenticated)?
        .as_str()
        .trim()
        .to_lowercase();
    let subject = claims.subject().as_str().to_string();
    if email.is_empty() || subject.is_empty() {
        return Err(AppError::Unauthenticated);
    }
    let name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| email.clone());
    let avatar_url = claims
        .picture()
        .and_then(|p| p.get(None))
        .filter(|p| p.as_str().starts_with("https://"))
        .map(|p| p.as_str().to_string());
    Ok(GoogleProfile {
        subject,
        email,
        name,
        avatar_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn auth() -> GoogleAuth {
        let config = Config {
            collaboration_url: None,
            collaboration_secret: None,
            pronunciation_url: None,
            translation_url: None,
            media_root: "unused".into(),
            google_client_id: "test-client".into(),
            google_client_secret: "test-secret".into(),
            google_redirect_url: "https://api.example.test/callback".into(),
            cookie_secure: true,
            bind_addr: "127.0.0.1:3000".into(),
            frontend_url: "https://frontend.example.test".into(),
            database_url: "unused".into(),
            database_max_connections: 1,
            database_acquire_timeout_secs: 1,
            jwt_private_key_path: "unused".into(),
            jwt_issuer: "https://api.example.test".into(),
            jwt_audience: "test".into(),
            jwt_key_id: "test".into(),
        };
        let metadata = serde_json::from_value(serde_json::json!({
            "issuer":"https://accounts.google.com",
            "authorization_endpoint":"https://accounts.google.com/authorize",
            "token_endpoint":"https://accounts.google.com/token",
            "jwks_uri":"https://accounts.google.com/jwks",
            "response_types_supported":["code"],"subject_types_supported":["public"],
            "id_token_signing_alg_values_supported":["RS256"]
        }))
        .unwrap();
        GoogleAuth::new(Arc::new(config))
            .unwrap()
            .with_provider_metadata(metadata)
            .unwrap()
    }
    #[tokio::test]
    async fn fresh_metadata_allows_parallel_authorization_while_another_reader_holds_the_cache() {
        let auth = auth();
        let guard = auth.metadata.read().await;
        let (a, b) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(auth.authorization(), auth.authorization())
        })
        .await
        .expect("fresh cache required an exclusive lock");
        let (a_url, a) = a.unwrap();
        let (b_url, b) = b.unwrap();
        assert_ne!(a.state.secret(), b.state.secret());
        assert_ne!(a.nonce, b.nonce);
        assert!(a_url.contains("code_challenge_method=S256"));
        assert!(b_url.contains("code_challenge_method=S256"));
        drop(guard);
    }
}
