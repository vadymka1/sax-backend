use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpaSectionTranslationDto {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpaSectionTranslationsDto {
    pub en: SpaSectionTranslationDto,
    pub de: Option<SpaSectionTranslationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    pub translations: SpaSectionTranslationsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSpaSectionTranslationRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSpaSectionTranslationsRequest {
    pub en: CreateSpaSectionTranslationRequest,
    pub de: Option<CreateSpaSectionTranslationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSpaSectionRequest {
    #[serde(default)]
    pub title: String,
    pub navigation_label: Option<String>,
    pub translations: Option<CreateSpaSectionTranslationsRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSpaSectionTranslationRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSpaSectionTranslationsRequest {
    pub en: Option<UpdateSpaSectionTranslationRequest>,
    pub de: Option<UpdateSpaSectionTranslationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSpaSectionRequest {
    pub title: Option<String>,
    pub navigation_label: Option<String>,
    pub is_visible: Option<bool>,
    pub translations: Option<UpdateSpaSectionTranslationsRequest>,
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
