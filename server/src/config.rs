use std::env;

#[derive(Clone)]
pub struct Config {
    pub collaboration_url: Option<String>,
    pub collaboration_secret: Option<String>,
    pub pronunciation_url: Option<String>,
    pub translation_url: Option<String>,
    pub media_root: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_url: String,
    pub cookie_secure: bool,
    pub bind_addr: String,
    pub frontend_url: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_acquire_timeout_secs: u64,
    pub jwt_private_key_path: String,
    pub jwt_issuer: String,
    pub jwt_audience: String,
    pub jwt_key_id: String,
}

impl Config {
    pub fn from_env() -> Self {
        let cookie_secure = optional("COOKIE_SECURE", "true")
            .parse::<bool>()
            .expect("COOKIE_SECURE phải là true hoặc false");
        let frontend_url = required("FRONTEND_URL").trim_end_matches('/').to_string();
        let google_redirect_url = required("GOOGLE_REDIRECT_URL");
        let jwt_issuer = required("JWT_ISSUER");
        for value in [&frontend_url, &google_redirect_url, &jwt_issuer] {
            let url = reqwest::Url::parse(value).expect("URL không hợp lệ");
            assert!(
                url.scheme() == "https"
                    || (url.scheme() == "http"
                        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))),
                "Chỉ cho phép HTTP ở localhost"
            );
            assert!(
                url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "URL không được chứa thông tin xác thực, query hoặc fragment"
            );
            if cookie_secure {
                assert!(
                    url.scheme() == "https",
                    "Cookie Secure cần HTTPS; đặt COOKIE_SECURE=false khi chạy localhost"
                );
            }
        }
        assert!(
            reqwest::Url::parse(&frontend_url).unwrap().path() == "/",
            "FRONTEND_URL phải là origin, không chứa đường dẫn"
        );
        let collaboration_url = env::var("COLLABORATION_URL").ok().filter(|v| !v.is_empty());
        let collaboration_secret = env::var("COLLABORATION_SECRET")
            .ok()
            .filter(|v| !v.is_empty());
        if let Some(value) = &collaboration_url {
            let url = reqwest::Url::parse(value).expect("COLLABORATION_URL không hợp lệ");
            assert!(
                url.scheme() == "wss"
                    || (url.scheme() == "ws"
                        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))),
                "WebSocket cần WSS, ngoại trừ localhost"
            );
            assert!(
                url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "WebSocket URL không được chứa thông tin xác thực"
            );
            assert!(
                collaboration_secret.as_ref().is_some_and(|s| s.len() >= 32),
                "COLLABORATION_SECRET cần ít nhất 32 ký tự"
            );
        }
        Self {
            pronunciation_url: service_url("PRONUNCIATION_URL"),
            translation_url: service_url("TRANSLATION_URL"),
            collaboration_url,
            collaboration_secret,
            media_root: optional("MEDIA_ROOT", ".storage/media"),
            google_client_id: required("GOOGLE_CLIENT_ID"),
            google_client_secret: required("GOOGLE_CLIENT_SECRET"),
            google_redirect_url,
            cookie_secure,
            bind_addr: optional("APP_BIND", "127.0.0.1:3000"),
            frontend_url,
            database_url: required("DATABASE_URL"),
            database_max_connections: positive("DATABASE_MAX_CONNECTIONS", 10)
                .try_into()
                .expect("DATABASE_MAX_CONNECTIONS quá lớn"),
            database_acquire_timeout_secs: positive("DATABASE_ACQUIRE_TIMEOUT_SECS", 5),
            jwt_private_key_path: required("JWT_PRIVATE_KEY_PATH"),
            jwt_issuer,
            jwt_audience: required("JWT_AUDIENCE"),
            jwt_key_id: required("JWT_KEY_ID"),
        }
    }
}

fn required(name: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("Chưa cấu hình biến môi trường {name}"))
}

fn optional(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn service_url(name: &str) -> Option<String> {
    let value = env::var(name).ok().filter(|s| !s.is_empty())?;
    let url = reqwest::Url::parse(&value).expect("Service URL không hợp lệ");
    assert!(
        url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))),
        "Service URL cần HTTPS hoặc localhost"
    );
    assert!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Service URL không được chứa thông tin xác thực"
    );
    Some(value.trim_end_matches('/').into())
}

fn positive(name: &str, default: u64) -> u64 {
    let value = optional(name, &default.to_string())
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name} phải là số nguyên dương"));
    assert!(value > 0, "{name} phải lớn hơn 0");
    value
}
