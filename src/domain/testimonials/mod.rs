use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_AUTHOR_NAME_LEN: usize = 120;
pub const MAX_AUTHOR_ROLE_LEN: usize = 160;
pub const MAX_TEXT_LEN: usize = 3000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Testimonial {
    pub id: Uuid,
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar_media_id: Option<Uuid>,
    pub sort_order: i32,
    pub is_visible: bool,
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
