use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_AUTHOR_NAME_LEN: usize = 120;
pub const MAX_AUTHOR_ROLE_LEN: usize = 160;
pub const MAX_TEXT_LEN: usize = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TestimonialModerationStatus {
    Pending,
    Approved,
    Rejected,
}

impl TestimonialModerationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

impl std::fmt::Display for TestimonialModerationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TestimonialSubmissionSource {
    Admin,
    Public,
}

impl TestimonialSubmissionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Public => "public",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "public" => Some(Self::Public),
            _ => None,
        }
    }
}

impl std::fmt::Display for TestimonialSubmissionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Testimonial {
    pub id: Uuid,
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar_media_id: Option<Uuid>,
    pub sort_order: i32,
    pub is_visible: bool,
    pub moderation_status: TestimonialModerationStatus,
    pub submission_source: TestimonialSubmissionSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub fn validate_author_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Author name cannot be empty".to_string());
    }
    if trimmed.chars().count() > MAX_AUTHOR_NAME_LEN {
        return Err(format!(
            "Author name exceeds maximum length of {} characters",
            MAX_AUTHOR_NAME_LEN
        ));
    }
    Ok(trimmed.to_string())
}

pub fn validate_author_role(role: Option<&str>) -> Result<Option<String>, String> {
    match role {
        Some(r) => {
            let trimmed = r.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.chars().count() > MAX_AUTHOR_ROLE_LEN {
                Err(format!(
                    "Author role exceeds maximum length of {} characters",
                    MAX_AUTHOR_ROLE_LEN
                ))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        None => Ok(None),
    }
}

pub fn validate_text(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Testimonial text cannot be empty".to_string());
    }
    if trimmed.chars().count() > MAX_TEXT_LEN {
        return Err(format!(
            "Testimonial text exceeds maximum length of {} characters",
            MAX_TEXT_LEN
        ));
    }
    Ok(trimmed.to_string())
}

pub fn validate_sort_order(sort_order: i32) -> Result<(), String> {
    if sort_order < 0 {
        return Err("sort_order cannot be negative".to_string());
    }
    Ok(())
}
