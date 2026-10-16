use crate::AppError;
use axum::extract::{
    rejection::{JsonRejection, PathRejection, QueryRejection},
    FromRequest, FromRequestParts,
};

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct AppQuery<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct AppPath<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(error: JsonRejection) -> Self {
        Self::validation("body", &error.body_text())
    }
}
impl From<QueryRejection> for AppError {
    fn from(error: QueryRejection) -> Self {
        Self::validation("query", &error.body_text())
    }
}
impl From<PathRejection> for AppError {
    fn from(_: PathRejection) -> Self {
        Self::NotFound("Không tìm thấy dữ liệu.".into())
    }
}
