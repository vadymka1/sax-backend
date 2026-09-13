use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ContactRequest {
    pub name: String,
    pub email: String,
    pub subject: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ContactResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminContactMessageDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub subject: Option<String>,
    pub message: String,
    pub email_status: String,
    pub email_error: Option<String>,
    pub email_sent_at: Option<DateTime<Utc>>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
