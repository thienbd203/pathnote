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
        let mut cache = self.metadata.write().await;
        if cache
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(300))
        {
            let metadata = CoreProviderMetadata::discover_async(
                IssuerUrl::new("https://accounts.google.com".into()).unwrap(),
                &self.http,
            )
            .await
            .map_err(|_| AppError::Internal("Google discovery unavailable".into()))?;
            *cache = Some((Instant::now(), metadata));
        }
        let metadata = cache.as_ref().unwrap().1.clone();
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
