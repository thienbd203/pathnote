use std::env;

#[derive(Clone)]
pub struct Config {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_url: String,
    pub cookie_secure: bool,
    pub bind_addr: String,
    pub frontend_url: String,
    pub database_url: String,
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
        Self {
            google_client_id: required("GOOGLE_CLIENT_ID"),
            google_client_secret: required("GOOGLE_CLIENT_SECRET"),
            google_redirect_url,
            cookie_secure,
            bind_addr: optional("APP_BIND", "127.0.0.1:3000"),
            frontend_url,
            database_url: required("DATABASE_URL"),
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
