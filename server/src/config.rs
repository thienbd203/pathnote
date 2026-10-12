use std::env;

pub struct Config {
    pub app_env: String,
    pub bind_addr: String,
    pub frontend_url: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub access_token_ttl_minutes: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let jwt_secret = required("JWT_SECRET");
        assert!(
            jwt_secret.len() >= 32,
            "JWT_SECRET phải dài ít nhất 32 ký tự"
        );

        Self {
            app_env: optional("APP_ENV", "production"),
            bind_addr: optional("APP_BIND", "127.0.0.1:3000"),
            frontend_url: required("FRONTEND_URL").trim_end_matches('/').to_string(),
            database_url: required("DATABASE_URL"),
            jwt_secret,
            access_token_ttl_minutes: optional("ACCESS_TOKEN_TTL_MINUTES", "15")
                .parse()
                .expect("ACCESS_TOKEN_TTL_MINUTES phải là số nguyên dương"),
        }
    }

    pub fn is_local(&self) -> bool {
        self.app_env == "local"
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Chưa cấu hình biến môi trường {name}"))
}

fn optional(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}
