use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::DbErr;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<std::collections::BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug)]
pub enum AppError {
    ClientOutdated,
    Unavailable(String),
    Conflict(String),
    ConflictWith {
        message: String,
        data: serde_json::Value,
    },
    MethodNotAllowed,
    RequestTimeout,
    Unauthenticated,
    Forbidden,
    BadRequest(String),
    NotFound(String),
    Internal(String),
    Validation(std::collections::BTreeMap<String, Vec<String>>),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut errors = None;
        let mut data = None;
        let mut code = None;
        let (status, message) = match self {
            AppError::ClientOutdated => {
                code = Some("client_outdated");
                (StatusCode::CONFLICT, "Có phiên bản mới, tải lại trang để lưu. Bản đang viết vẫn được giữ; hãy sao chép hoặc tải bản nháp trước khi tải lại.".into())
            }
            AppError::Validation(fields) => {
                errors = Some(fields);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Dữ liệu không hợp lệ".into(),
                )
            }
            AppError::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::ConflictWith {
                message,
                data: value,
            } => {
                data = Some(value);
                (StatusCode::CONFLICT, message)
            }
            AppError::RequestTimeout => (
                StatusCode::GATEWAY_TIMEOUT,
                "Yêu cầu quá thời gian chờ. Bạn hãy thử lại.".into(),
            ),
            AppError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "Phương thức không được hỗ trợ.".into(),
            ),
            AppError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "Bạn cần đăng nhập để tiếp tục".to_string(),
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Nguồn yêu cầu không hợp lệ".into()),
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            AppError::Internal(detail) => {
                tracing::error!(%detail, "Lỗi nội bộ");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Đã có lỗi xảy ra, vui lòng thử lại sau".to_string(),
                )
            }
        };

        let body = ErrorBody {
            message,
            code,
            errors,
            data,
        };

        (status, Json(body)).into_response()
    }
}

impl From<DbErr> for AppError {
    fn from(err: DbErr) -> Self {
        AppError::Internal(err.to_string())
    }
}

pub async fn route_not_found() -> AppError {
    AppError::NotFound("Không tìm thấy đường dẫn".to_string())
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientOutdated => f.write_str("ClientOutdated"),
            Self::MethodNotAllowed => f.write_str("MethodNotAllowed"),
            Self::RequestTimeout => f.write_str("RequestTimeout"),
            Self::Unauthenticated => f.write_str("Unauthenticated"),
            Self::Forbidden => f.write_str("Forbidden"),
            Self::Unavailable(message) => write!(f, "Unavailable: {message}"),
            Self::Conflict(message) => write!(f, "Conflict: {message}"),
            Self::ConflictWith { message, .. } => write!(f, "ConflictWith: {message}"),
            Self::BadRequest(message) => write!(f, "BadRequest: {message}"),
            Self::NotFound(message) => write!(f, "NotFound: {message}"),
            Self::Internal(message) => write!(f, "Internal: {message}"),
            Self::Validation(fields) => write!(f, "Validation: {fields:?}"),
        }
    }
}
impl std::error::Error for AppError {}

impl AppError {
    pub fn validation(field: &str, message: &str) -> Self {
        Self::Validation(std::collections::BTreeMap::from([(
            field.to_owned(),
            vec![message.to_owned()],
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_identifies_errors_without_logging_conflict_document_data() {
        assert_eq!(AppError::Unauthenticated.to_string(), "Unauthenticated");
        assert_eq!(
            AppError::Internal("failed".into()).to_string(),
            "Internal: failed"
        );
        let conflict = AppError::ConflictWith {
            message: "changed".into(),
            data: serde_json::json!({"secret":"private note"}),
        };
        assert_eq!(conflict.to_string(), "ConflictWith: changed");
        assert!(!conflict.to_string().contains("private note"));
        assert!(AppError::validation("name", "required")
            .to_string()
            .starts_with("Validation:"));
    }
    #[tokio::test]
    async fn route_timeout_is_gateway_timeout_with_the_existing_json_message() {
        let response = AppError::RequestTimeout.into_response();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body["message"],
            "Yêu cầu quá thời gian chờ. Bạn hãy thử lại."
        );
    }
}
