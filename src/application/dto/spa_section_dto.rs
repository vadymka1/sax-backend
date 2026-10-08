use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Localized representation of a SpaSection in a specific locale.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpaSectionTranslationDto {
    /// Localized section display name/title.
    pub name: String,
    /// Localized navigation label for menu display.
    pub navigation_label: Option<String>,
}

/// Admin translations container for a SpaSection.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpaSectionTranslationsDto {
    /// Canonical English translation (always present).
    pub en: SpaSectionTranslationDto,
    /// German translation (present only if created).
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

/// Request payload to create localized section content.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSpaSectionTranslationRequest {
    /// Localized section name.
    pub name: String,
    /// Localized navigation label. Optional; defaults to English name if omitted for EN.
    #[serde(default)]
    pub navigation_label: Option<String>,
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

/// Request payload to update localized section content. Supports partial updates per locale.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSpaSectionTranslationRequest {
    /// Optional updated section name. Omitted leaves name unchanged.
    #[serde(default)]
    pub name: Option<String>,
    /// Optional updated navigation label. Omitted leaves label unchanged.
    #[serde(default)]
    pub navigation_label: Option<String>,
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
