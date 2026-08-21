use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::sections::ContentBlockType;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockAttachedMediaDto {
    pub id: Uuid,
    pub media_type: String,
    pub storage_provider: String,
    pub original_filename: Option<String>,
    pub stored_filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub youtube_url: Option<String>,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AdminContentBlockDto {
    pub id: Uuid,
    pub spa_section_id: Uuid,
    pub section_key: String,
    pub section_title: String,
    pub block_type: ContentBlockType,
    pub title: Option<String>,
    pub text: String,
    pub media: Option<BlockAttachedMediaDto>,
    pub sort_order: i32,
    pub is_visible: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateContentBlockRequest {
    pub spa_section_id: Uuid,
    pub block_type: ContentBlockType,
    pub title: Option<String>,
    pub text: String,
    pub media_id: Option<Uuid>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateContentBlockRequest {
    pub spa_section_id: Option<Uuid>,
    pub block_type: Option<ContentBlockType>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub media_id: Option<Option<Uuid>>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderContentBlockItem {
    pub id: Uuid,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderContentBlocksRequest {
    pub items: Vec<ReorderContentBlockItem>,
}
