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
    errors: Option<std::collections::BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug)]
pub enum AppError {
    Unavailable(String),
    Conflict(String),
    ConflictWith {
        message: String,
        data: serde_json::Value,
    },
    MethodNotAllowed,
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
        let (status, message) = match self {
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
        f.write_str("Authentication or application error")
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
