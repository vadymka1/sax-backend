use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdminMediaDto {
    Image {
        id: Uuid,
        url: String,
        original_filename: Option<String>,
        mime_type: String,
        file_size: i64,
        alt_text: Option<String>,
        created_at: DateTime<Utc>,
    },
    Video {
        id: Uuid,
        url: String,
        original_filename: Option<String>,
        mime_type: String,
        file_size: i64,
        created_at: DateTime<Utc>,
    },
    Youtube {
        id: Uuid,
        youtube_video_id: String,
        youtube_url: String,
        embed_url: String,
        thumbnail_url: String,
        title: Option<String>,
        created_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MediaResponseDto {
    pub data: AdminMediaDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UploadMediaRequest {
    /// Image or video media file
    #[schema(value_type = String, format = Binary)]
    pub file: String,
}
