use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::pages::{
    BackgroundMode, BackgroundPosition, BackgroundSize, PageAppearanceSettings,
};
use crate::shared::errors::{AppError, AppResult};

#[derive(Debug, sqlx::FromRow)]
pub struct AppearanceWithMediaRow {
    pub id: Uuid,
    pub page_id: Uuid,
    pub background_mode: String,
    pub background_color: String,
    pub overlay_opacity: f64,
    pub background_position: String,
    pub background_size: String,
    pub media_id: Option<Uuid>,
    pub media_type: Option<String>,
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub media_created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct SettingsRow {
    id: Uuid,
    page_id: Uuid,
    background_mode: String,
    background_color: String,
    background_media_id: Option<Uuid>,
    overlay_opacity: f64,
    background_position: String,
    background_size: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub struct PageAppearanceRepository<'a> {
    pool: &'a PgPool,
}

#[derive(Debug, Clone)]
pub struct UpsertAppearanceParams<'a> {
    pub page_id: Uuid,
    pub background_mode: BackgroundMode,
    pub background_color: &'a str,
    pub background_media_id: Option<Uuid>,
    pub overlay_opacity: f64,
    pub background_position: BackgroundPosition,
    pub background_size: BackgroundSize,
}

impl<'a> PageAppearanceRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Fetches the appearance settings with a single JOIN to media_assets.
    /// Inactive or deleted media assets are treated as NULL by the JOIN condition.
    pub async fn get_by_page_id(&self, page_id: Uuid) -> AppResult<Option<AppearanceWithMediaRow>> {
        let row = sqlx::query_as::<_, AppearanceWithMediaRow>(
            r#"
            SELECT 
                pas.id,
                pas.page_id,
                pas.background_mode,
                pas.background_color,
                pas.overlay_opacity,
                pas.background_position,
                pas.background_size,
                m.id AS media_id,
                m.media_type,
                m.storage_key,
                m.original_filename,
                m.mime_type,
                m.file_size,
                m.alt_text,
                m.created_at AS media_created_at
            FROM page_appearance_settings pas
            LEFT JOIN media_assets m ON pas.background_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE pas.page_id = $1
            "#,
        )
        .bind(page_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(row)
    }

    /// Fetches the raw settings record for a page (used for partial updates).
    pub async fn get_raw_settings(
        &self,
        page_id: Uuid,
    ) -> AppResult<Option<PageAppearanceSettings>> {
        let row = sqlx::query_as::<_, SettingsRow>(
            r#"
            SELECT id, page_id, background_mode, background_color, background_media_id, overlay_opacity, background_position, background_size, created_at, updated_at
            FROM page_appearance_settings
            WHERE page_id = $1
            "#,
        )
        .bind(page_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| PageAppearanceSettings {
            id: r.id,
            page_id: r.page_id,
            background_mode: BackgroundMode::parse(&r.background_mode).unwrap_or_default(),
            background_color: r.background_color,
            background_media_id: r.background_media_id,
            overlay_opacity: r.overlay_opacity,
            background_position: BackgroundPosition::parse(&r.background_position)
                .unwrap_or_default(),
            background_size: BackgroundSize::parse(&r.background_size).unwrap_or_default(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    /// Atomically upserts the page appearance settings record.
    pub async fn upsert(
        &self,
        params: UpsertAppearanceParams<'_>,
    ) -> AppResult<PageAppearanceSettings> {
        let row = sqlx::query_as::<_, SettingsRow>(
            r#"
            INSERT INTO page_appearance_settings (
                page_id, background_mode, background_color, background_media_id, overlay_opacity, background_position, background_size, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP)
            ON CONFLICT (page_id) DO UPDATE SET
                background_mode = EXCLUDED.background_mode,
                background_color = EXCLUDED.background_color,
                background_media_id = EXCLUDED.background_media_id,
                overlay_opacity = EXCLUDED.overlay_opacity,
                background_position = EXCLUDED.background_position,
                background_size = EXCLUDED.background_size,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, page_id, background_mode, background_color, background_media_id, overlay_opacity, background_position, background_size, created_at, updated_at
            "#,
        )
        .bind(params.page_id)
        .bind(params.background_mode.as_str())
        .bind(params.background_color)
        .bind(params.background_media_id)
        .bind(params.overlay_opacity)
        .bind(params.background_position.as_str())
        .bind(params.background_size.as_str())
        .fetch_one(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(PageAppearanceSettings {
            id: row.id,
            page_id: row.page_id,
            background_mode: BackgroundMode::parse(&row.background_mode).unwrap_or_default(),
            background_color: row.background_color,
            background_media_id: row.background_media_id,
            overlay_opacity: row.overlay_opacity,
            background_position: BackgroundPosition::parse(&row.background_position)
                .unwrap_or_default(),
            background_size: BackgroundSize::parse(&row.background_size).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}
