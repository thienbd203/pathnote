use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    #[serde(skip)]
    status: StatusCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn created(data: T) -> Self {
        let mut response = Self::ok(data);
        response.status = StatusCode::CREATED;
        response
    }
    pub fn ok(data: T) -> Self {
        Self {
            status: StatusCode::OK,
            message: None,
            data: Some(data),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}
