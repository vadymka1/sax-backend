use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminContentBlockDto, BlockAttachedMediaDto, CreateContentBlockRequest,
    ReorderContentBlocksRequest, UpdateContentBlockRequest,
};
use crate::domain::sections::ContentBlockType;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub const DEFAULT_MEDIA_USAGE_TYPE: &str = "content";

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
struct SectionRow {
    id: Uuid,
    spa_section_id: Uuid,
    section_type: String,
    title: Option<String>,
    content: serde_json::Value,
    sort_order: i32,
    is_visible: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
struct MediaRow {
    id: Uuid,
    media_type: String,
    storage_provider: String,
    original_filename: Option<String>,
    stored_filename: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
    youtube_url: Option<String>,
    thumbnail_url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct JoinedAdminBlockRow {
    id: Uuid,
    spa_section_id: Uuid,
    section_key: String,
    section_title: String,
    section_type: String,
    title: Option<String>,
    content: serde_json::Value,
    sort_order: i32,
    is_visible: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    media_id: Option<Uuid>,
    media_type: Option<String>,
    storage_provider: Option<String>,
    original_filename: Option<String>,
    stored_filename: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
    youtube_url: Option<String>,
    thumbnail_url: Option<String>,
}

pub struct ContentBlockService<'a> {
    pool: &'a PgPool,
}

impl<'a> ContentBlockService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    async fn get_home_page_id_tx(tx: &mut Transaction<'_, Postgres>) -> AppResult<Uuid> {
        let res: (Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Home page not found".to_string()))?;
        Ok(res.0)
    }

    async fn get_home_page_id(&self) -> AppResult<Uuid> {
        let res: (Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
            .fetch_optional(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Home page not found".to_string()))?;
        Ok(res.0)
    }

    fn validate_text_and_title(&self, title: Option<&str>, text: &str) -> AppResult<()> {
        if let Some(t) = title {
            if t.len() > 200 {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "title".to_string(),
                    message: "Title exceeds 200 characters".to_string(),
                }]));
            }
        }
        if text.trim().is_empty() {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "text".to_string(),
                message: "Text cannot be empty or whitespace".to_string(),
            }]));
        }
        if text.len() > 10000 {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "text".to_string(),
                message: "Text exceeds maximum length".to_string(),
            }]));
        }
        Ok(())
    }

    async fn validate_media_for_type_tx(
        tx: &mut Transaction<'_, Postgres>,
        block_type: ContentBlockType,
        media_id: Option<Uuid>,
    ) -> AppResult<Option<BlockAttachedMediaDto>> {
        match block_type {
            ContentBlockType::Text => {
                if media_id.is_some() {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "Text block cannot have media".to_string(),
                    }]));
                }
                Ok(None)
            }
            ContentBlockType::TextImage => {
                let mid = media_id.ok_or_else(|| {
                    AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_image requires media_id".to_string(),
                    }])
                })?;
                let m = Self::fetch_active_media_tx(tx, mid).await?;
                if m.media_type != "image" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_image block requires media of type image".to_string(),
                    }]));
                }
                Ok(Some(m))
            }
            ContentBlockType::TextYoutube => {
                let mid = media_id.ok_or_else(|| {
                    AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_youtube requires media_id".to_string(),
                    }])
                })?;
                let m = Self::fetch_active_media_tx(tx, mid).await?;
                if m.media_type != "youtube" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_youtube block requires media of type youtube".to_string(),
                    }]));
                }
                Ok(Some(m))
            }
            ContentBlockType::TextVideo => {
                let mid = media_id.ok_or_else(|| {
                    AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_video requires media_id".to_string(),
                    }])
                })?;
                let m = Self::fetch_active_media_tx(tx, mid).await?;
                if m.media_type != "video" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_id".to_string(),
                        message: "text_video block requires media of type video".to_string(),
                    }]));
                }
                Ok(Some(m))
            }
        }
    }

    async fn fetch_active_media_tx(
        tx: &mut Transaction<'_, Postgres>,
        media_id: Uuid,
    ) -> AppResult<BlockAttachedMediaDto> {
        let row = sqlx::query_as::<_, MediaRow>(
            "SELECT id, media_type, storage_provider, original_filename, stored_filename, mime_type, file_size, youtube_url, thumbnail_url FROM media_assets WHERE id = $1 AND deleted_at IS NULL AND status = 'active'"
        )
        .bind(media_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::ValidationError(vec![ApiErrorDetails {
            field: "media_id".to_string(),
            message: "Media asset not found or inactive".to_string(),
        }]))?;

        Ok(BlockAttachedMediaDto {
            id: row.id,
            media_type: row.media_type,
            storage_provider: row.storage_provider,
            original_filename: row.original_filename,
            stored_filename: row.stored_filename,
            mime_type: row.mime_type,
            file_size: row.file_size,
            youtube_url: row.youtube_url,
            thumbnail_url: row.thumbnail_url,
        })
    }

    async fn validate_and_lock_spa_section_tx(
        tx: &mut Transaction<'_, Postgres>,
        spa_section_id: Uuid,
    ) -> AppResult<(String, String)> {
        let res: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT ss.section_key, ss.title
            FROM spa_sections ss
            JOIN pages p ON p.id = ss.page_id
            WHERE ss.id = $1 AND p.slug = 'home' AND ss.deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(spa_section_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        res.ok_or_else(|| {
            AppError::NotFound(
                "Referenced SPA section does not exist, is deleted, or does not belong to the home page".to_string(),
            )
        })
    }

    async fn get_next_sort_order_for_section_tx(
        tx: &mut Transaction<'_, Postgres>,
        spa_section_id: Uuid,
    ) -> AppResult<i32> {
        let max_res: (i32,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM sections WHERE spa_section_id = $1 AND deleted_at IS NULL",
        )
        .bind(spa_section_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(max_res.0)
    }

    async fn get_home_block_for_update_tx(
        tx: &mut Transaction<'_, Postgres>,
        block_id: Uuid,
    ) -> AppResult<JoinedAdminBlockRow> {
        let row = sqlx::query_as::<_, JoinedAdminBlockRow>(
            r#"
            SELECT
                s.id,
                s.spa_section_id,
                ss.section_key,
                ss.title AS section_title,
                s.section_type,
                s.title,
                s.content,
                s.sort_order,
                s.is_visible,
                s.created_at,
                s.updated_at,
                m.id AS media_id,
                m.media_type,
                m.storage_provider,
                m.original_filename,
                m.stored_filename,
                m.mime_type,
                m.file_size,
                m.youtube_url,
                m.thumbnail_url
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            FOR UPDATE OF s
            "#,
        )
        .bind(block_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Content block not found".to_string()))?;

        Ok(row)
    }

    pub async fn create_block(
        &self,
        auth: &AuthenticatedUser,
        req: CreateContentBlockRequest,
    ) -> AppResult<AdminContentBlockDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        self.validate_text_and_title(req.title.as_deref(), &req.text)?;

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let (sec_key, sec_title) =
            Self::validate_and_lock_spa_section_tx(&mut tx, req.spa_section_id).await?;
        let sort_order =
            Self::get_next_sort_order_for_section_tx(&mut tx, req.spa_section_id).await?;

        let media_dto =
            Self::validate_media_for_type_tx(&mut tx, req.block_type, req.media_id).await?;
        let page_id = Self::get_home_page_id_tx(&mut tx).await?;
        let block_id = Uuid::new_v4();
        let section_key = format!("block_{}", block_id.simple());
        let content_json = serde_json::json!({ "text": req.text });
        let is_visible = req.is_visible.unwrap_or(true);

        let row = sqlx::query_as::<_, SectionRow>(
            r#"
            INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content, sort_order, is_visible, status, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'published', $10)
            RETURNING id, spa_section_id, section_type, title, content, sort_order, is_visible, created_at, updated_at
            "#
        )
        .bind(block_id)
        .bind(page_id)
        .bind(req.spa_section_id)
        .bind(section_key)
        .bind(req.block_type.as_str())
        .bind(&req.title)
        .bind(&content_json)
        .bind(sort_order)
        .bind(is_visible)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if let Some(ref m) = media_dto {
            let res = sqlx::query(
                "INSERT INTO section_media (id, section_id, media_asset_id, usage_type) VALUES ($1, $2, $3, $4)"
            )
            .bind(Uuid::new_v4())
            .bind(block_id)
            .bind(m.id)
            .bind(DEFAULT_MEDIA_USAGE_TYPE)
            .execute(&mut *tx)
            .await;

            if let Err(e) = res {
                let _ = tx.rollback().await;
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.code().as_deref() == Some("23505") {
                        return Err(AppError::ResourceConflict(
                            "Content block already has an attached media asset".to_string(),
                        ));
                    }
                }
                return Err(AppError::DatabaseError(e.to_string()));
            }
        }

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(AdminContentBlockDto {
            id: row.id,
            spa_section_id: row.spa_section_id,
            section_key: sec_key,
            section_title: sec_title,
            block_type: req.block_type,
            title: row.title,
            text: req.text,
            media: media_dto,
            sort_order: row.sort_order,
            is_visible: row.is_visible,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn list_blocks(
        &self,
        auth: &AuthenticatedUser,
        filter_spa_section_id: Option<Uuid>,
    ) -> AppResult<Vec<AdminContentBlockDto>> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let page_id = self.get_home_page_id().await?;

        if let Some(filter_id) = filter_spa_section_id {
            // Verify filter target section exists and belongs to home page
            let sec_exists: Option<(bool,)> = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM spa_sections ss JOIN pages p ON p.id = ss.page_id WHERE ss.id = $1 AND p.slug = 'home' AND ss.deleted_at IS NULL)"
            )
            .bind(filter_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            if !sec_exists.map(|r| r.0).unwrap_or(false) {
                return Err(AppError::NotFound("SPA Section not found".to_string()));
            }

            let rows = sqlx::query_as::<_, JoinedAdminBlockRow>(
                r#"
                SELECT
                    s.id,
                    s.spa_section_id,
                    ss.section_key,
                    ss.title AS section_title,
                    s.section_type,
                    s.title,
                    s.content,
                    s.sort_order,
                    s.is_visible,
                    s.created_at,
                    s.updated_at,
                    m.id AS media_id,
                    m.media_type,
                    m.storage_provider,
                    m.original_filename,
                    m.stored_filename,
                    m.mime_type,
                    m.file_size,
                    m.youtube_url,
                    m.thumbnail_url
                FROM sections s
                JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
                JOIN pages p ON p.id = s.page_id
                LEFT JOIN section_media sm ON s.id = sm.section_id
                LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
                WHERE s.spa_section_id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
                ORDER BY s.sort_order ASC, s.id ASC
                "#
            )
            .bind(filter_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            return Ok(Self::map_rows_to_dtos(rows));
        }

        // Unfiltered list ordered by SpaSection.sort_order ASC, ContentBlock.sort_order ASC, ContentBlock.id ASC
        let rows = sqlx::query_as::<_, JoinedAdminBlockRow>(
            r#"
            SELECT
                s.id,
                s.spa_section_id,
                ss.section_key,
                ss.title AS section_title,
                s.section_type,
                s.title,
                s.content,
                s.sort_order,
                s.is_visible,
                s.created_at,
                s.updated_at,
                m.id AS media_id,
                m.media_type,
                m.storage_provider,
                m.original_filename,
                m.stored_filename,
                m.mime_type,
                m.file_size,
                m.youtube_url,
                m.thumbnail_url
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.page_id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            ORDER BY ss.sort_order ASC, s.sort_order ASC, s.id ASC
            "#
        )
        .bind(page_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(Self::map_rows_to_dtos(rows))
    }

    fn map_rows_to_dtos(rows: Vec<JoinedAdminBlockRow>) -> Vec<AdminContentBlockDto> {
        let mut dtos = Vec::new();
        for row in rows {
            let block_type = match ContentBlockType::parse(&row.section_type) {
                Some(bt) => bt,
                None => continue,
            };
            let text = row
                .content
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let media = row.media_id.map(|mid| BlockAttachedMediaDto {
                id: mid,
                media_type: row.media_type.unwrap_or_default(),
                storage_provider: row.storage_provider.unwrap_or_default(),
                original_filename: row.original_filename,
                stored_filename: row.stored_filename,
                mime_type: row.mime_type,
                file_size: row.file_size,
                youtube_url: row.youtube_url,
                thumbnail_url: row.thumbnail_url,
            });

            dtos.push(AdminContentBlockDto {
                id: row.id,
                spa_section_id: row.spa_section_id,
                section_key: row.section_key,
                section_title: row.section_title,
                block_type,
                title: row.title,
                text,
                media,
                sort_order: row.sort_order,
                is_visible: row.is_visible,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
        dtos
    }

    pub async fn get_block(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminContentBlockDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let row = sqlx::query_as::<_, JoinedAdminBlockRow>(
            r#"
            SELECT
                s.id,
                s.spa_section_id,
                ss.section_key,
                ss.title AS section_title,
                s.section_type,
                s.title,
                s.content,
                s.sort_order,
                s.is_visible,
                s.created_at,
                s.updated_at,
                m.id AS media_id,
                m.media_type,
                m.storage_provider,
                m.original_filename,
                m.stored_filename,
                m.mime_type,
                m.file_size,
                m.youtube_url,
                m.thumbnail_url
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            "#
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Content block not found".to_string()))?;

        let block_type = ContentBlockType::parse(&row.section_type)
            .ok_or_else(|| AppError::Internal("Invalid block type".to_string()))?;
        let text = row
            .content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let media = row.media_id.map(|mid| BlockAttachedMediaDto {
            id: mid,
            media_type: row.media_type.unwrap_or_default(),
            storage_provider: row.storage_provider.unwrap_or_default(),
            original_filename: row.original_filename,
            stored_filename: row.stored_filename,
            mime_type: row.mime_type,
            file_size: row.file_size,
            youtube_url: row.youtube_url,
            thumbnail_url: row.thumbnail_url,
        });

        Ok(AdminContentBlockDto {
            id: row.id,
            spa_section_id: row.spa_section_id,
            section_key: row.section_key,
            section_title: row.section_title,
            block_type,
            title: row.title,
            text,
            media,
            sort_order: row.sort_order,
            is_visible: row.is_visible,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn update_block(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
        req: UpdateContentBlockRequest,
    ) -> AppResult<AdminContentBlockDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Transactionally load and lock current home block inside tx
        let current_row = Self::get_home_block_for_update_tx(&mut tx, id).await?;

        let current_block_type = ContentBlockType::parse(&current_row.section_type)
            .ok_or_else(|| AppError::Internal("Invalid block type".to_string()))?;

        let current_text = current_row
            .content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Section Move Logic
        let (target_spa_section_id, new_sec_key, new_sec_title, target_sort_order) =
            if let Some(target_sec_id) = req.spa_section_id {
                if target_sec_id != current_row.spa_section_id {
                    // Lock sections in deterministic UUID ASC order to prevent deadlocks
                    let (sec1, sec2) = if current_row.spa_section_id < target_sec_id {
                        (current_row.spa_section_id, target_sec_id)
                    } else {
                        (target_sec_id, current_row.spa_section_id)
                    };

                    sqlx::query("SELECT id FROM spa_sections WHERE id = $1 FOR UPDATE")
                        .bind(sec1)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

                    if sec1 != sec2 {
                        sqlx::query("SELECT id FROM spa_sections WHERE id = $1 FOR UPDATE")
                            .bind(sec2)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
                    }

                    // Validate target section
                    let (target_key, target_title) =
                        Self::validate_and_lock_spa_section_tx(&mut tx, target_sec_id).await?;

                    // Compute target sort order (append to end of target section)
                    let new_order =
                        Self::get_next_sort_order_for_section_tx(&mut tx, target_sec_id).await?;

                    (target_sec_id, target_key, target_title, new_order)
                } else {
                    (
                        current_row.spa_section_id,
                        current_row.section_key.clone(),
                        current_row.section_title.clone(),
                        current_row.sort_order,
                    )
                }
            } else {
                (
                    current_row.spa_section_id,
                    current_row.section_key.clone(),
                    current_row.section_title.clone(),
                    current_row.sort_order,
                )
            };

        let new_block_type = req.block_type.unwrap_or(current_block_type);
        let new_title = match req.title {
            Some(t) => Some(t),
            None => current_row.title,
        };
        let new_text = req.text.unwrap_or(current_text);
        let new_is_visible = req.is_visible.unwrap_or(current_row.is_visible);

        self.validate_text_and_title(new_title.as_deref(), &new_text)?;

        // Media Preservation Logic:
        // Distinguish: media_id not supplied in req vs media_id explicitly supplied
        let media_mutation_requested = req.media_id.is_some() || req.block_type.is_some();
        let target_media_id = match req.media_id {
            Some(opt_mid) => opt_mid,
            None => current_row.media_id,
        };

        let media_dto =
            Self::validate_media_for_type_tx(&mut tx, new_block_type, target_media_id).await?;
        let content_json = serde_json::json!({ "text": new_text });

        let updated_row = sqlx::query_as::<_, SectionRow>(
            r#"
            UPDATE sections
            SET spa_section_id = $1, section_type = $2, title = $3, content = $4, sort_order = $5, is_visible = $6, updated_at = CURRENT_TIMESTAMP, updated_by = $7
            WHERE id = $8 AND deleted_at IS NULL
            RETURNING id, spa_section_id, section_type, title, content, sort_order, is_visible, created_at, updated_at
            "#
        )
        .bind(target_spa_section_id)
        .bind(new_block_type.as_str())
        .bind(&new_title)
        .bind(&content_json)
        .bind(target_sort_order)
        .bind(new_is_visible)
        .bind(auth.id)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Mutate section_media ONLY if media or block_type was explicitly changed
        if media_mutation_requested {
            let media_changed =
                target_media_id != current_row.media_id || new_block_type != current_block_type;

            if media_changed {
                sqlx::query("DELETE FROM section_media WHERE section_id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

                if let Some(ref m) = media_dto {
                    let res = sqlx::query(
                        "INSERT INTO section_media (id, section_id, media_asset_id, usage_type) VALUES ($1, $2, $3, $4)"
                    )
                    .bind(Uuid::new_v4())
                    .bind(id)
                    .bind(m.id)
                    .bind(DEFAULT_MEDIA_USAGE_TYPE)
                    .execute(&mut *tx)
                    .await;

                    if let Err(e) = res {
                        let _ = tx.rollback().await;
                        if let sqlx::Error::Database(ref db_err) = e {
                            if db_err.code().as_deref() == Some("23505") {
                                return Err(AppError::ResourceConflict(
                                    "Content block already has an attached media asset".to_string(),
                                ));
                            }
                        }
                        return Err(AppError::DatabaseError(e.to_string()));
                    }
                }
            }
        }

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(AdminContentBlockDto {
            id: updated_row.id,
            spa_section_id: updated_row.spa_section_id,
            section_key: new_sec_key,
            section_title: new_sec_title,
            block_type: new_block_type,
            title: updated_row.title,
            text: new_text,
            media: media_dto,
            sort_order: updated_row.sort_order,
            is_visible: updated_row.is_visible,
            created_at: updated_row.created_at,
            updated_at: updated_row.updated_at,
        })
    }

    pub async fn delete_block(&self, auth: &AuthenticatedUser, id: Uuid) -> AppResult<()> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Verify home isolation and lock FOR UPDATE
        let block_exists: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT s.id
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            WHERE s.id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            FOR UPDATE OF s
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if block_exists.is_none() {
            return Err(AppError::NotFound("Content block not found".to_string()));
        }

        sqlx::query("UPDATE sections SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn reorder_section_blocks(
        &self,
        auth: &AuthenticatedUser,
        spa_section_id: Uuid,
        req: ReorderContentBlocksRequest,
    ) -> AppResult<()> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Lock SpaSection row FOR UPDATE inside transaction verifying home page ownership
        let sec_row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT ss.id
            FROM spa_sections ss
            JOIN pages p ON p.id = ss.page_id
            WHERE ss.id = $1 AND p.slug = 'home' AND ss.deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(spa_section_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if sec_row.is_none() {
            return Err(AppError::NotFound("SPA Section not found".to_string()));
        }

        // Fetch active block IDs belonging to this section
        let active_rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM sections WHERE spa_section_id = $1 AND deleted_at IS NULL ORDER BY sort_order ASC, id ASC",
        )
        .bind(spa_section_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let db_ids_set: HashSet<Uuid> = active_rows.into_iter().map(|r| r.0).collect();

        // Empty section reorder handling: if DB has 0 blocks and req.items is empty -> success
        if db_ids_set.is_empty() && req.items.is_empty() {
            tx.commit()
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            return Ok(());
        }

        if req.items.len() != db_ids_set.len() {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "items".to_string(),
                message: format!(
                    "Reorder items count ({}) does not match active blocks count ({}) for section",
                    req.items.len(),
                    db_ids_set.len()
                ),
            }]));
        }

        let mut req_ids_set = HashSet::new();
        let mut req_orders_set = HashSet::new();

        for item in &req.items {
            if item.sort_order < 0 {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("sort_order cannot be negative: {}", item.sort_order),
                }]));
            }

            if !db_ids_set.contains(&item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!(
                        "Referenced ContentBlock ID {} does not belong to this section or is deleted",
                        item.id
                    ),
                }]));
            }

            if !req_ids_set.insert(item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("Duplicate block ID in reorder request: {}", item.id),
                }]));
            }

            if !req_orders_set.insert(item.sort_order) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!(
                        "Duplicate sort_order in reorder request: {}",
                        item.sort_order
                    ),
                }]));
            }
        }

        // Two-phase reorder: Phase 1 assign temporary negative order offset
        for (idx, item) in req.items.iter().enumerate() {
            let temp_order = -100000 - (idx as i32);
            sqlx::query(
                "UPDATE sections SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 AND spa_section_id = $3 AND deleted_at IS NULL",
            )
            .bind(temp_order)
            .bind(item.id)
            .bind(spa_section_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Phase 2: assign final target sort order
        for item in &req.items {
            sqlx::query(
                "UPDATE sections SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 AND spa_section_id = $3 AND deleted_at IS NULL",
            )
            .bind(item.sort_order)
            .bind(item.id)
            .bind(spa_section_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
