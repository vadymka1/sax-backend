use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct AdminSpaSectionDto {
    pub id: Uuid,
    pub key: String,
    pub title: String,
    pub navigation_label: String,
    pub sort_order: i32,
    pub is_visible: bool,
    pub content_block_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSpaSectionRequest {
    pub title: String,
    pub navigation_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSpaSectionRequest {
    pub title: Option<String>,
    pub navigation_label: Option<String>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderSpaSectionItem {
    pub id: Uuid,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderSpaSectionsRequest {
    pub items: Vec<ReorderSpaSectionItem>,
}
