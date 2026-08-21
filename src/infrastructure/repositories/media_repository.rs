use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::media::{MediaType, YoutubeUrlParser};
use crate::shared::errors::{AppError, AppResult};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MediaAssetRecord {
    pub id: Uuid,
    pub media_type: String,
    pub storage_provider: String,
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub stored_filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub youtube_video_id: Option<String>,
    pub youtube_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct MediaRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> MediaRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_youtube_asset(
        &self,
        youtube_url: &str,
        title: Option<&str>,
        caption: Option<&str>,
        alt_text: Option<&str>,
        created_by: Uuid,
    ) -> AppResult<MediaAssetRecord> {
        let video_id =
            YoutubeUrlParser::parse_id(youtube_url).ok_or(AppError::InvalidYoutubeUrl)?;
        let thumbnail = YoutubeUrlParser::build_thumbnail_url(&video_id);

        let id = Uuid::new_v4();
        sqlx::query_as::<_, MediaAssetRecord>(
            r#"
            INSERT INTO media_assets (id, media_type, storage_provider, youtube_video_id, youtube_url, thumbnail_url, alt_text, caption, created_by)
            VALUES ($1, $2, 'local', $3, $4, $5, $6, $7, $8)
            RETURNING id, media_type, storage_provider, storage_key, original_filename, stored_filename, mime_type, file_size, alt_text, caption, youtube_video_id, youtube_url, thumbnail_url, status, created_at
            "#
        )
        .bind(id)
        .bind(MediaType::Youtube.as_str())
        .bind(&video_id)
        .bind(youtube_url)
        .bind(thumbnail)
        .bind(alt_text)
        .bind(caption.or(title))
        .bind(created_by)
        .fetch_one(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }
}
