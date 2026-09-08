use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    #[default]
    Sans,
    Serif,
    Display,
    Mono,
}

impl FontFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            FontFamily::Sans => "sans",
            FontFamily::Serif => "serif",
            FontFamily::Display => "display",
            FontFamily::Mono => "mono",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sans" => Some(FontFamily::Sans),
            "serif" => Some(FontFamily::Serif),
            "display" => Some(FontFamily::Display),
            "mono" => Some(FontFamily::Mono),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
pub enum FontSize {
    #[serde(rename = "sm")]
    Sm,
    #[default]
    #[serde(rename = "md")]
    Md,
    #[serde(rename = "lg")]
    Lg,
    #[serde(rename = "xl")]
    Xl,
    #[serde(rename = "2xl")]
    TwoXl,
}

impl FontSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            FontSize::Sm => "sm",
            FontSize::Md => "md",
            FontSize::Lg => "lg",
            FontSize::Xl => "xl",
            FontSize::TwoXl => "2xl",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sm" => Some(FontSize::Sm),
            "md" => Some(FontSize::Md),
            "lg" => Some(FontSize::Lg),
            "xl" => Some(FontSize::Xl),
            "2xl" => Some(FontSize::TwoXl),
            _ => None,
        }
    }
}
