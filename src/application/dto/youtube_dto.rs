use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct YoutubeMediaResponseDto {
    pub id: Uuid,
    pub r#type: String,
    pub youtube_video_id: String,
    pub youtube_url: String,
    pub embed_url: String,
    pub thumbnail_url: String,
}
