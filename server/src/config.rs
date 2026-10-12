use std::env;

pub struct Config {
    pub bind_addr: String,
    pub frontend_url: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind_addr: optional("APP_BIND", "127.0.0.1:3000"),
            frontend_url: required("FRONTEND_URL").trim_end_matches('/').to_string(),
            database_url: required("DATABASE_URL"),
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Chưa cấu hình biến môi trường {name}"))
}

fn optional(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}
