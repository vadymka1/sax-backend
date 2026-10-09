use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    #[default]
    None,
    Color,
    Image,
}

impl BackgroundMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackgroundMode::None => "none",
            BackgroundMode::Color => "color",
            BackgroundMode::Image => "image",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(BackgroundMode::None),
            "color" => Some(BackgroundMode::Color),
            "image" => Some(BackgroundMode::Image),
            _ => None,
        }
    }
}

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

/// Validates and normalizes a CSS hex color.
/// Only 6-character hex strings with '#' prefix (e.g. `#FFFFFF`, `#f4efe8`) are valid.
/// Normalizes to uppercase `#RRGGBB`.
pub fn validate_and_normalize_hex_color(color: &str) -> Result<String, String> {
    let trimmed = color.trim();
    if trimmed.len() != 7 || !trimmed.starts_with('#') {
        return Err(
            "Background color must be a valid 6-character hex code starting with # (e.g. #FFFFFF)"
                .to_string(),
        );
    }
    let hex_digits = &trimmed[1..];
    if !hex_digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Background color contains invalid hexadecimal characters".to_string());
    }
    Ok(format!("#{}", hex_digits.to_ascii_uppercase()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageAppearanceSettings {
    pub id: Uuid,
    pub page_id: Uuid,
    pub background_mode: BackgroundMode,
    pub background_color: String,
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
            background_mode: BackgroundMode::None,
            background_color: "#FFFFFF".to_string(),
            background_media_id: None,
            overlay_opacity: 0.35,
            background_position: BackgroundPosition::Center,
            background_size: BackgroundSize::Cover,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
