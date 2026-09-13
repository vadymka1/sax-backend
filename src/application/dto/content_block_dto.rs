use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::sections::{ContentBlockType, FontFamily, FontSize};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BlockAttachedMediaDto {
    pub id: Uuid,
    #[serde(alias = "type")]
    pub media_type: String,
    pub storage_provider: String,
    pub original_filename: Option<String>,
    pub stored_filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub youtube_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub url: Option<String>,
    pub alt_text: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminContentBlockDto {
    pub id: Uuid,
    pub spa_section_id: Uuid,
    pub section_key: String,
    pub section_title: String,
    pub block_type: ContentBlockType,
    pub title: Option<String>,
    pub text: String,
    pub media: Vec<BlockAttachedMediaDto>,
    pub font_family: FontFamily,
    pub font_size: FontSize,
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
    pub media_ids: Option<Vec<Uuid>>,
    pub font_family: Option<FontFamily>,
    pub font_size: Option<FontSize>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateContentBlockRequest {
    pub spa_section_id: Option<Uuid>,
    pub block_type: Option<ContentBlockType>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub media_id: Option<Option<Uuid>>,
    pub media_ids: Option<Option<Vec<Uuid>>>,
    pub font_family: Option<FontFamily>,
    pub font_size: Option<FontSize>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReorderContentBlockItem {
    pub id: Uuid,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReorderContentBlocksRequest {
    pub items: Vec<ReorderContentBlockItem>,
}
