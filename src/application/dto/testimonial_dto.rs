use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::testimonials::{TestimonialModerationStatus, TestimonialSubmissionSource};

pub fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminTestimonialAvatarDto {
    pub id: Uuid,
    pub url: String,
    pub alt_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminTestimonialDto {
    pub id: Uuid,
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar: Option<AdminTestimonialAvatarDto>,
    pub sort_order: i32,
    pub is_visible: bool,
    pub moderation_status: TestimonialModerationStatus,
    pub submission_source: TestimonialSubmissionSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SubmitPublicTestimonialRequest {
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitPublicTestimonialResponse {
    pub id: Uuid,
    pub status: TestimonialModerationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTestimonialRequest {
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar_media_id: Option<Uuid>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateTestimonialRequest {
    pub author_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub author_role: Option<Option<String>>,
    pub text: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    #[schema(value_type = Option<Uuid>, nullable = true)]
    pub avatar_media_id: Option<Option<Uuid>>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderTestimonialItem {
    pub id: Uuid,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderTestimonialsRequest {
    pub items: Vec<ReorderTestimonialItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicTestimonialAvatarDto {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PublicTestimonialDto {
    pub id: Uuid,
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar: Option<PublicTestimonialAvatarDto>,
    pub sort_order: i32,
}
