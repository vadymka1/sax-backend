use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod content_block_dto;
pub mod media_dto;
pub mod public_dto;
pub mod spa_section_dto;
pub mod youtube_dto;
pub use content_block_dto::*;
pub use media_dto::*;
pub use public_dto::*;
pub use spa_section_dto::*;
pub use youtube_dto::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    #[schema(example = "admin@example.com")]
    pub email: String,
    #[schema(example = "Super Admin User")]
    pub display_name: String,
    #[schema(example = "super_admin")]
    pub role: String,
    pub is_active: bool,
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthTokensDto {
    pub access_token: String,
    pub refresh_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    #[schema(example = 900)]
    pub expires_in: i64,
    pub user: UserDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RefreshTokenDataDto {
    pub access_token: String,
    pub refresh_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    #[schema(example = 900)]
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageDataDto {
    #[schema(example = "Operation completed successfully")]
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "admin@example.com")]
    pub email: String,
    #[schema(example = "secretpassword")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    #[schema(example = "a1b2c3d4e5f67890123456789012345678901234567890123456789012345678")]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LogoutRequest {
    #[schema(example = "a1b2c3d4e5f67890123456789012345678901234567890123456789012345678")]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(example = "new_admin@example.com")]
    pub email: String,
    #[schema(example = "SecurePassword123!")]
    pub password: String,
    #[schema(example = "Jane Admin")]
    pub display_name: String,
    /// User role: "admin" or "super_admin" (defaults to "admin")
    #[schema(example = "admin")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    #[schema(example = "Updated Name")]
    pub display_name: Option<String>,
    /// User role: "admin" or "super_admin"
    #[schema(example = "admin")]
    pub role: Option<String>,
    #[schema(example = true)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSectionRequest {
    pub section_key: String,
    pub section_type: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderItem {
    pub id: Uuid,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderRequest {
    pub items: Vec<ReorderItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateYoutubeMediaRequest {
    #[schema(example = "https://www.youtube.com/watch?v=dQw4w9WgXcQ")]
    pub youtube_url: String,
    #[schema(example = "Official Video")]
    pub title: Option<String>,
    pub caption: Option<String>,
    pub alt_text: Option<String>,
}
