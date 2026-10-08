use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::sections::{ContentBlockType, FontFamily, FontSize};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicPageDto {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub seo_keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PublicMediaDto {
    Image {
        id: Uuid,
        url: String,
        alt_text: Option<String>,
    },
    Video {
        id: Uuid,
        url: String,
        mime_type: Option<String>,
    },
    Youtube {
        id: Uuid,
        youtube_video_id: String,
        embed_url: String,
        thumbnail_url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicContentBlockDto {
    pub id: Uuid,
    pub block_type: ContentBlockType,
    pub title: Option<String>,
    pub text: String,
    pub media: Vec<PublicMediaDto>,
    pub font_family: FontFamily,
    pub font_size: FontSize,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicSpaSectionDto {
    pub id: Uuid,
    pub key: String,
    /// Localized section title with fallback (requested locale -> English).
    pub title: String,
    /// Localized navigation label with 5-tier fallback:
    /// (1) requested locale nav -> (2) English nav -> (3) requested locale name -> (4) English name -> (5) legacy navigation_label.
    pub navigation_label: String,
    pub sort_order: i32,
    pub blocks: Vec<PublicContentBlockDto>,
}

use crate::application::dto::page_appearance_dto::PublicPageAppearanceDto;
use crate::application::dto::testimonial_dto::PublicTestimonialDto;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicPageResponse {
    pub page: PublicPageDto,
    pub appearance: PublicPageAppearanceDto,
    pub sections: Vec<PublicSpaSectionDto>,
    pub testimonials: Vec<PublicTestimonialDto>,
}
