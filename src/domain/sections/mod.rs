pub mod spa_section;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use spa_section::{generate_slug, validate_spa_section_key, SpaSection};

pub type SectionType = ContentBlockType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentBlockType {
    Text,
    TextImage,
    TextYoutube,
    TextVideo,
}

impl ContentBlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentBlockType::Text => "text",
            ContentBlockType::TextImage => "text_image",
            ContentBlockType::TextYoutube => "text_youtube",
            ContentBlockType::TextVideo => "text_video",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(ContentBlockType::Text),
            "text_image" => Some(ContentBlockType::TextImage),
            "text_youtube" => Some(ContentBlockType::TextYoutube),
            "text_video" => Some(ContentBlockType::TextVideo),
            _ => None,
        }
    }
}
