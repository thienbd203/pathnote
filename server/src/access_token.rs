use axum::{extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::jwk::{Jwk, PublicKeyUse};
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{error::AppError, state::AppState};

pub const ACCESS_TOKEN_TTL: u64 = 15 * 60;

#[derive(Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
    token_use: String,
}

pub struct AccessTokens {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    issuer: String,
    audience: String,
    key_id: String,
    jwks: Value,
}

impl AccessTokens {
    pub fn from_pem(
        pem: &str,
        issuer: String,
        audience: String,
        key_id: String,
    ) -> Result<Self, AppError> {
        let encoding = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|_| AppError::Internal("Invalid access token signing key".into()))?;
        let mut jwk = Jwk::from_encoding_key(&encoding, Algorithm::RS256)
            .map_err(|_| AppError::Internal("Cannot export public key".into()))?;
        jwk.common.key_id = Some(key_id.clone());
        jwk.common.public_key_use = Some(PublicKeyUse::Signature);
        let decoding = DecodingKey::from_jwk(&jwk)
            .map_err(|_| AppError::Internal("Invalid public key".into()))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_required_spec_claims(&["sub", "iss", "aud", "exp"]);
        validation.set_issuer(&[&issuer]);
        validation.set_audience(&[&audience]);
        validation.leeway = 0;
        Ok(Self {
            encoding,
            decoding,
            validation,
            issuer,
            audience,
            key_id,
            jwks: json!({"keys": [jwk]}),
        })
    }

    pub fn issue(&self, user_id: i64) -> Result<String, AppError> {
        let now = jsonwebtoken::get_current_timestamp();
        let mut header = Header::new(Algorithm::RS256);
        header.typ = Some("at+jwt".into());
        header.kid = Some(self.key_id.clone());
        encode(
            &header,
            &Claims {
                sub: user_id.to_string(),
                iss: self.issuer.clone(),
                aud: self.audience.clone(),
                iat: now,
                exp: now + ACCESS_TOKEN_TTL,
                token_use: "access".into(),
            },
            &self.encoding,
        )
        .map_err(|_| AppError::Internal("Cannot issue access token".into()))
    }

    fn verify(&self, token: &str) -> Result<i64, AppError> {
        let header = decode_header(token).map_err(|_| AppError::Unauthenticated)?;
        if header.typ.as_deref() != Some("at+jwt") || header.kid.as_deref() != Some(&self.key_id) {
            return Err(AppError::Unauthenticated);
        }
        let claims = decode::<Claims>(token, &self.decoding, &self.validation)
            .map_err(|_| AppError::Unauthenticated)?
            .claims;
        let now = jsonwebtoken::get_current_timestamp();
        if claims.token_use != "access"
            || claims.iat > now
            || claims.exp <= now
            || claims.exp <= claims.iat
            || claims.exp - claims.iat > ACCESS_TOKEN_TTL
        {
            return Err(AppError::Unauthenticated);
        }
        claims
            .sub
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or(AppError::Unauthenticated)
    }

    pub fn jwks(&self) -> Value {
        self.jwks.clone()
    }
}

pub struct CurrentUser {
    pub id: i64,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let value = parts
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::Unauthenticated)?;
        let (scheme, token) = value.split_once(' ').ok_or(AppError::Unauthenticated)?;
        if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() || token.len() > 8192 {
            return Err(AppError::Unauthenticated);
        }
        Ok(Self {
            id: state.access_tokens.verify(token)?,
        })
    }
}
