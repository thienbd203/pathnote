use crate::AppError;
use serde::{Deserialize, Serialize};
fn first_page() -> u64 {
    1
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    #[serde(default = "first_page")]
    pub page: u64,
    pub per_page: Option<u64>,
    pub q: Option<String>,
}
impl ListQuery {
    pub fn page_size(&self) -> Result<u64, AppError> {
        if self.page == 0 || self.page > i32::MAX as u64 {
            return Err(AppError::validation("page", "Trang không hợp lệ."));
        }
        let size = self.per_page.unwrap_or(25);
        if !(1..=100).contains(&size) {
            return Err(AppError::validation(
                "per_page",
                "Số mục mỗi trang từ 1 đến 100.",
            ));
        }
        if self.q.as_ref().is_some_and(|q| q.chars().count() > 120) {
            return Err(AppError::validation("q", "Từ khóa tối đa 120 ký tự."));
        }
        Ok(size)
    }
}
#[derive(Serialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub meta: PageMeta,
}
#[derive(Serialize)]
pub struct PageMeta {
    pub current_page: u64,
    pub last_page: u64,
    pub per_page: u64,
    pub total: u64,
}
