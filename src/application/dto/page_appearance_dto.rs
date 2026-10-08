use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::application::dto::media_dto::AdminMediaDto;
use crate::application::dto::public_dto::PublicMediaDto;
use crate::domain::pages::{BackgroundPosition, BackgroundSize};

use crate::application::dto::testimonial_dto::deserialize_optional_field;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminPageAppearanceDto {
    pub background_media: Option<AdminMediaDto>,
    pub overlay_opacity: f64,
    pub background_position: BackgroundPosition,
    pub background_size: BackgroundSize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicPageAppearanceDto {
    pub background_media: Option<PublicMediaDto>,
    pub overlay_opacity: f64,
    pub background_position: BackgroundPosition,
    pub background_size: BackgroundSize,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdatePageAppearanceRequest {
    /// Optional UUID of the background media asset.
    /// Provide a valid UUID string to assign an image asset.
    /// Provide `null` to explicitly remove the current background image.
    /// Omit the field to leave the current background unchanged.
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub background_media_id: Option<Option<Uuid>>,
    /// Overlay opacity numeric value between 0.0 and 1.0 (default 0.35)
    pub overlay_opacity: Option<f64>,
    /// Background position: "center", "top", or "bottom"
    pub background_position: Option<BackgroundPosition>,
    /// Background size: "cover" or "contain"
    pub background_size: Option<BackgroundSize>,
}
