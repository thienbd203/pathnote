use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::DbErr;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    success: bool,
    message: String,
}

#[derive(Debug)]
pub enum AppError {
    Unauthenticated,
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "Bạn cần đăng nhập để tiếp tục".to_string(),
            ),
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
            success: false,
            message,
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
