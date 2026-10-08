use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BackgroundPosition {
    #[default]
    Center,
    Top,
    Bottom,
}

impl BackgroundPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackgroundPosition::Center => "center",
            BackgroundPosition::Top => "top",
            BackgroundPosition::Bottom => "bottom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "center" => Some(BackgroundPosition::Center),
            "top" => Some(BackgroundPosition::Top),
            "bottom" => Some(BackgroundPosition::Bottom),
            _ => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BackgroundSize {
    #[default]
    Cover,
    Contain,
}

impl BackgroundSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackgroundSize::Cover => "cover",
            BackgroundSize::Contain => "contain",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cover" => Some(BackgroundSize::Cover),
            "contain" => Some(BackgroundSize::Contain),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageAppearanceSettings {
    pub id: Uuid,
    pub page_id: Uuid,
    pub background_media_id: Option<Uuid>,
    pub overlay_opacity: f64,
    pub background_position: BackgroundPosition,
    pub background_size: BackgroundSize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for PageAppearanceSettings {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            page_id: Uuid::nil(),
            background_media_id: None,
            overlay_opacity: 0.35,
            background_position: BackgroundPosition::Center,
            background_size: BackgroundSize::Cover,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
