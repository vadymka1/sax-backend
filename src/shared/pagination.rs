use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub page: i64,
    pub page_size: i64,
    pub total_items: i64,
    pub total_pages: i64,
}

impl PaginationMeta {
    pub fn new(page: i64, page_size: i64, total_items: i64) -> Self {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let total_pages = (total_items as f64 / page_size as f64).ceil() as i64;
        Self {
            page,
            page_size,
            total_items,
            total_pages: total_pages.max(1),
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SingleResponse<T> {
    pub data: T,
}
