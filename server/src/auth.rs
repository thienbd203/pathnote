use axum::{extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{
    decode, encode, get_current_timestamp, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};

use crate::{error::AppError, state::AppState};

const ACCESS_TOKEN: &str = "access";

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    iat: u64,
    exp: u64,
    token_use: String,
}

pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    access_ttl_secs: u64,
}

impl JwtKeys {
    pub fn new(secret: &str, access_ttl_minutes: u64) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "sub"]);

        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            validation,
            access_ttl_secs: access_ttl_minutes * 60,
        }
    }

    pub fn access_ttl_secs(&self) -> u64 {
        self.access_ttl_secs
    }

    pub fn issue_access_token(&self, user_id: i64) -> Result<String, AppError> {
        let now = get_current_timestamp();

        let claims = Claims {
            sub: user_id.to_string(),
            iat: now,
            exp: now + self.access_ttl_secs,
            token_use: ACCESS_TOKEN.to_string(),
        };

        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|err| AppError::Internal(err.to_string()))
    }

    fn verify_access_token(&self, token: &str) -> Result<i64, AppError> {
        let claims = decode::<Claims>(token, &self.decoding, &self.validation)
            .map_err(|_| AppError::Unauthenticated)?
            .claims;

        if claims.token_use != ACCESS_TOKEN {
            return Err(AppError::Unauthenticated);
        }

        claims
            .sub
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or(AppError::Unauthenticated)
    }
}

/// Người dùng đã xác thực. Handler nào khai báo tham số này thì bắt buộc đăng nhập.
pub struct CurrentUser {
    pub id: i64,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(AppError::Unauthenticated)?;
        let id = state.jwt.verify_access_token(token)?;
        Ok(CurrentUser { id })
    }
}

fn bearer_token(parts: &Parts) -> Option<&str> {
    let header = parts.headers.get("authorization")?.to_str().ok()?;
    let (scheme, token) = header.split_once(' ')?;
    let token = token.trim();

    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}
