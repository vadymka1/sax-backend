use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::AdminMediaDto;
use crate::config::AppConfig;
use crate::domain::media::YoutubeUrlParser;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

#[derive(sqlx::FromRow)]
struct MediaRowFull {
    id: Uuid,
    media_type: String,
    storage_key: Option<String>,
    original_filename: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
    alt_text: Option<String>,
    youtube_video_id: Option<String>,
    youtube_url: Option<String>,
    thumbnail_url: Option<String>,
    title: Option<String>,
    created_at: DateTime<Utc>,
}

pub struct MediaService<'a> {
    pool: &'a PgPool,
    config: &'a AppConfig,
    storage: Arc<dyn StorageProvider>,
}

impl<'a> MediaService<'a> {
    pub fn new(pool: &'a PgPool, config: &'a AppConfig, storage: Arc<dyn StorageProvider>) -> Self {
        Self {
            pool,
            config,
            storage,
        }
    }

    fn map_row_to_dto(&self, row: MediaRowFull) -> AppResult<AdminMediaDto> {
        match row.media_type.as_str() {
            "image" => {
                let key = row.storage_key.unwrap_or_default();
                let url = self.storage.get_public_url(&key);
                Ok(AdminMediaDto::Image {
                    id: row.id,
                    url,
                    original_filename: row.original_filename,
                    mime_type: row.mime_type.unwrap_or_else(|| "image/jpeg".to_string()),
                    file_size: row.file_size.unwrap_or(0),
                    alt_text: row.alt_text,
                    created_at: row.created_at,
                })
            }
            "video" => {
                let key = row.storage_key.unwrap_or_default();
                let url = self.storage.get_public_url(&key);
                Ok(AdminMediaDto::Video {
                    id: row.id,
                    url,
                    original_filename: row.original_filename,
                    mime_type: row.mime_type.unwrap_or_else(|| "video/mp4".to_string()),
                    file_size: row.file_size.unwrap_or(0),
                    created_at: row.created_at,
                })
            }
            "youtube" => {
                let yid = row.youtube_video_id.unwrap_or_default();
                let canonical_url = row
                    .youtube_url
                    .unwrap_or_else(|| YoutubeUrlParser::build_canonical_url(&yid));
                let embed_url = YoutubeUrlParser::build_embed_url(&yid);
                let thumbnail_url = row
                    .thumbnail_url
                    .unwrap_or_else(|| YoutubeUrlParser::build_thumbnail_url(&yid));

                Ok(AdminMediaDto::Youtube {
                    id: row.id,
                    youtube_video_id: yid,
                    youtube_url: canonical_url,
                    embed_url,
                    thumbnail_url,
                    title: row.original_filename.or(row.title),
                    created_at: row.created_at,
                })
            }
            _ => Err(AppError::Internal(format!(
                "Unsupported media type {}",
                row.media_type
            ))),
        }
    }

    pub async fn create_youtube_media(
        &self,
        auth: &AuthenticatedUser,
        req: crate::application::dto::CreateYoutubeMediaRequest,
    ) -> AppResult<AdminMediaDto> {
        if !auth.is_active || !auth.role.can_manage_media() {
            return Err(AppError::Forbidden);
        }

        let video_id =
            YoutubeUrlParser::parse_id(&req.youtube_url).ok_or(AppError::InvalidYoutubeUrl)?;

        let canonical_url = YoutubeUrlParser::build_canonical_url(&video_id);
        let thumbnail_url = YoutubeUrlParser::build_thumbnail_url(&video_id);
        let media_id = Uuid::new_v4();

        let row = sqlx::query_as::<_, MediaRowFull>(
            r#"
            INSERT INTO media_assets (id, media_type, storage_provider, youtube_video_id, youtube_url, thumbnail_url, original_filename, created_by)
            VALUES ($1, 'youtube', 'youtube', $2, $3, $4, $5, $6)
            RETURNING id, media_type, storage_key, original_filename, mime_type, file_size, alt_text, youtube_video_id, youtube_url, thumbnail_url, original_filename AS title, created_at
            "#
        )
        .bind(media_id)
        .bind(&video_id)
        .bind(&canonical_url)
        .bind(&thumbnail_url)
        .bind(req.title.as_deref())
        .bind(auth.id)
        .fetch_one(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        self.map_row_to_dto(row)
    }

    pub async fn process_uploaded_file(
        &self,
        auth: &AuthenticatedUser,
        temp_file_path: &Path,
        original_filename: Option<&str>,
        client_content_type: &str,
        file_size: u64,
    ) -> AppResult<AdminMediaDto> {
        if !auth.is_active || !auth.role.can_manage_media() {
            let _ = tokio::fs::remove_file(temp_file_path).await;
            return Err(AppError::Forbidden);
        }

        if file_size == 0 {
            let _ = tokio::fs::remove_file(temp_file_path).await;
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "file".to_string(),
                message: "Uploaded file is empty".to_string(),
            }]));
        }

        // Magic Bytes type detection using infer
        let kind = match infer::get_from_path(temp_file_path) {
            Ok(Some(k)) => k,
            _ => {
                let _ = tokio::fs::remove_file(temp_file_path).await;
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "file".to_string(),
                    message: "Unsupported file type or unreadable magic bytes".to_string(),
                }]));
            }
        };

        let detected_mime = kind.mime_type();
        let (media_type, extension, max_size_bytes) = match detected_mime {
            "image/jpeg" => (
                "image",
                "jpg",
                self.config.max_image_upload_mb * 1024 * 1024,
            ),
            "image/png" => (
                "image",
                "png",
                self.config.max_image_upload_mb * 1024 * 1024,
            ),
            "image/webp" => (
                "image",
                "webp",
                self.config.max_image_upload_mb * 1024 * 1024,
            ),
            "video/mp4" => (
                "video",
                "mp4",
                self.config.max_video_upload_mb * 1024 * 1024,
            ),
            "video/webm" => (
                "video",
                "webm",
                self.config.max_video_upload_mb * 1024 * 1024,
            ),
            _ => {
                let _ = tokio::fs::remove_file(temp_file_path).await;
                return Err(AppError::InvalidMediaType);
            }
        };

        // Validate compatibility with declared client content-type if provided
        if !client_content_type.is_empty()
            && client_content_type != "application/octet-stream"
            && !client_content_type.contains(detected_mime)
        {
            let _ = tokio::fs::remove_file(temp_file_path).await;
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "file".to_string(),
                message: "Declared content-type mismatch with detected magic bytes".to_string(),
            }]));
        }

        if file_size > max_size_bytes {
            let _ = tokio::fs::remove_file(temp_file_path).await;
            return Err(AppError::FileTooLarge);
        }

        let now = Utc::now();
        let file_uuid = Uuid::new_v4();
        let storage_key = format!(
            "{}s/{}/{}/{}.{}",
            media_type,
            now.format("%Y"),
            now.format("%m"),
            file_uuid,
            extension
        );

        let stored_filename = format!("{}.{}", file_uuid, extension);

        let result = sqlx::query_as::<_, MediaRowFull>(
            r#"
            INSERT INTO media_assets (id, media_type, storage_provider, storage_key, original_filename, stored_filename, mime_type, file_size, created_by)
            VALUES ($1, $2, 'local', $3, $4, $5, $6, $7, $8)
            RETURNING id, media_type, storage_key, original_filename, mime_type, file_size, alt_text, youtube_video_id, youtube_url, thumbnail_url, original_filename AS title, created_at
            "#
        )
        .bind(file_uuid)
        .bind(media_type)
        .bind(&storage_key)
        .bind(original_filename)
        .bind(&stored_filename)
        .bind(detected_mime)
        .bind(file_size as i64)
        .bind(auth.id)
        .fetch_one(self.pool)
        .await;

        let row = match result {
            Ok(r) => r,
            Err(e) => {
                let _ = tokio::fs::remove_file(temp_file_path).await;
                return Err(AppError::DatabaseError(e.to_string()));
            }
        };

        if let Err(e) = self.storage.move_file(temp_file_path, &storage_key).await {
            match sqlx::query("DELETE FROM media_assets WHERE id = $1")
                .bind(file_uuid)
                .execute(self.pool)
                .await
            {
                Ok(_) => {
                    tracing::warn!(
                        media_id = %file_uuid,
                        media_type = %media_type,
                        storage_error = %e,
                        "media storage failed; database row rolled back by compensation"
                    );
                }
                Err(cleanup_err) => {
                    tracing::error!(
                        media_id = %file_uuid,
                        media_type = %media_type,
                        storage_error = %e,
                        cleanup_error = %cleanup_err,
                        "media storage failed and compensating database cleanup also failed"
                    );
                }
            }
            let _ = tokio::fs::remove_file(temp_file_path).await;
            return Err(e);
        }

        self.map_row_to_dto(row)
    }

    pub async fn list_media(&self, auth: &AuthenticatedUser) -> AppResult<Vec<AdminMediaDto>> {
        if !auth.is_active || !auth.role.can_manage_media() {
            return Err(AppError::Forbidden);
        }

        let rows = sqlx::query_as::<_, MediaRowFull>(
            r#"
            SELECT id, media_type, storage_key, original_filename, mime_type, file_size, alt_text, youtube_video_id, youtube_url, thumbnail_url, original_filename AS title, created_at
            FROM media_assets
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut dtos = Vec::new();
        for r in rows {
            dtos.push(self.map_row_to_dto(r)?);
        }

        Ok(dtos)
    }

    pub async fn get_media(&self, auth: &AuthenticatedUser, id: Uuid) -> AppResult<AdminMediaDto> {
        if !auth.is_active || !auth.role.can_manage_media() {
            return Err(AppError::Forbidden);
        }

        let row = sqlx::query_as::<_, MediaRowFull>(
            r#"
            SELECT id, media_type, storage_key, original_filename, mime_type, file_size, alt_text, youtube_video_id, youtube_url, thumbnail_url, original_filename AS title, created_at
            FROM media_assets
            WHERE id = $1 AND deleted_at IS NULL
            "#
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound("Media asset not found".to_string()))?;

        self.map_row_to_dto(row)
    }

    pub async fn delete_media(&self, auth: &AuthenticatedUser, id: Uuid) -> AppResult<()> {
        if !auth.is_active || !auth.role.can_manage_media() {
            return Err(AppError::Forbidden);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM section_media WHERE media_asset_id = $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if count.0 > 0 {
            let _ = tx.rollback().await;
            return Err(AppError::ResourceConflict(
                "Media asset is currently attached to a content block".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, MediaRowFull>(
            r#"
            SELECT id, media_type, storage_key, original_filename, mime_type, file_size, alt_text, youtube_video_id, youtube_url, thumbnail_url, original_filename AS title, created_at
            FROM media_assets
            WHERE id = $1 AND deleted_at IS NULL
            "#
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound("Media asset not found".to_string()))?;

        sqlx::query("UPDATE media_assets SET deleted_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if let Some(key) = row.storage_key {
            let _ = self.storage.delete(&key).await;
        }

        Ok(())
    }
}
