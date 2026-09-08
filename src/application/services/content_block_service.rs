use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminContentBlockDto, BlockAttachedMediaDto, CreateContentBlockRequest,
    ReorderContentBlockItem, ReorderContentBlocksRequest, UpdateContentBlockRequest,
};
use crate::domain::sections::{ContentBlockType, FontFamily, FontSize};
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
    font_family: String,
    font_size: String,
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
    alt_text: Option<String>,
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
    font_family: String,
    font_size: String,
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
    alt_text: Option<String>,
    youtube_url: Option<String>,
    thumbnail_url: Option<String>,
    media_sort_order: Option<i32>,
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
                message: "Text exceeds 10000 characters".to_string(),
            }]));
        }
        Ok(())
    }

    async fn validate_media_for_type_tx(
        tx: &mut Transaction<'_, Postgres>,
        block_type: ContentBlockType,
        media_ids: &[Uuid],
    ) -> AppResult<Vec<BlockAttachedMediaDto>> {
        match block_type {
            ContentBlockType::Text => {
                if !media_ids.is_empty() {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: "Text block cannot have media".to_string(),
                    }]));
                }
                Ok(Vec::new())
            }
            ContentBlockType::TextImage => {
                if media_ids.is_empty() {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: "text_image block requires at least one image media asset"
                            .to_string(),
                    }]));
                }

                let mut seen = HashSet::new();
                let mut dtos = Vec::with_capacity(media_ids.len());

                for (idx, &mid) in media_ids.iter().enumerate() {
                    if !seen.insert(mid) {
                        return Err(AppError::ValidationError(vec![ApiErrorDetails {
                            field: "media_ids".to_string(),
                            message: format!("Duplicate media asset ID in request: {}", mid),
                        }]));
                    }
                    let mut m = Self::fetch_active_media_tx(tx, mid).await?;
                    if m.media_type != "image" {
                        return Err(AppError::ValidationError(vec![ApiErrorDetails {
                            field: "media_ids".to_string(),
                            message: format!(
                                "text_image block requires media of type image, got '{}'",
                                m.media_type
                            ),
                        }]));
                    }
                    m.sort_order = (idx as i32 + 1) * 10;
                    dtos.push(m);
                }
                Ok(dtos)
            }
            ContentBlockType::TextYoutube => {
                if media_ids.len() != 1 {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: "text_youtube requires exactly one youtube media asset"
                            .to_string(),
                    }]));
                }
                let mut m = Self::fetch_active_media_tx(tx, media_ids[0]).await?;
                if m.media_type != "youtube" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: format!(
                            "text_youtube block requires media of type youtube, got '{}'",
                            m.media_type
                        ),
                    }]));
                }
                m.sort_order = 10;
                Ok(vec![m])
            }
            ContentBlockType::TextVideo => {
                if media_ids.len() != 1 {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: "text_video requires exactly one video media asset".to_string(),
                    }]));
                }
                let mut m = Self::fetch_active_media_tx(tx, media_ids[0]).await?;
                if m.media_type != "video" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "media_ids".to_string(),
                        message: format!(
                            "text_video block requires media of type video, got '{}'",
                            m.media_type
                        ),
                    }]));
                }
                m.sort_order = 10;
                Ok(vec![m])
            }
        }
    }

    async fn fetch_active_media_tx(
        tx: &mut Transaction<'_, Postgres>,
        media_id: Uuid,
    ) -> AppResult<BlockAttachedMediaDto> {
        let row = sqlx::query_as::<_, MediaRow>(
            "SELECT id, media_type, storage_provider, original_filename, stored_filename, mime_type, file_size, alt_text, youtube_url, thumbnail_url FROM media_assets WHERE id = $1 AND deleted_at IS NULL AND status = 'active'"
        )
        .bind(media_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::ValidationError(vec![ApiErrorDetails {
            field: "media_id".to_string(),
            message: format!("Media asset not found or inactive: {}", media_id),
        }]))?;

        let url = if row.storage_provider == "youtube" {
            row.youtube_url.clone()
        } else {
            row.stored_filename
                .as_ref()
                .map(|f| format!("/uploads/{}", f))
        };

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
            url,
            alt_text: row.alt_text,
            sort_order: 0,
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

        match res {
            Some(vals) => Ok(vals),
            None => Err(AppError::NotFound("SPA Section not found".to_string())),
        }
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
    ) -> AppResult<Vec<JoinedAdminBlockRow>> {
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
                s.font_family,
                s.font_size,
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
                m.alt_text,
                m.youtube_url,
                m.thumbnail_url,
                sm.sort_order AS media_sort_order
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            ORDER BY sm.sort_order ASC, sm.created_at ASC, sm.id ASC
            FOR UPDATE OF s
            "#,
        )
        .bind(block_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if rows.is_empty() {
            return Err(AppError::NotFound("Content block not found".to_string()));
        }

        Ok(rows)
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

        // Support both media_ids array and legacy media_id
        let resolved_media_ids = if let Some(ref ids) = req.media_ids {
            ids.clone()
        } else if let Some(mid) = req.media_id {
            vec![mid]
        } else {
            Vec::new()
        };

        let media_dtos =
            Self::validate_media_for_type_tx(&mut tx, req.block_type, &resolved_media_ids).await?;
        let page_id = Self::get_home_page_id_tx(&mut tx).await?;
        let block_id = Uuid::new_v4();
        let section_key = format!("block_{}", block_id.simple());
        let content_json = serde_json::json!({ "text": req.text });
        let is_visible = req.is_visible.unwrap_or(true);

        let font_family = req.font_family.unwrap_or(FontFamily::Sans);
        let font_size = req.font_size.unwrap_or(FontSize::Md);

        let row = sqlx::query_as::<_, SectionRow>(
            r#"
            INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content, font_family, font_size, sort_order, is_visible, status, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'published', $12)
            RETURNING id, spa_section_id, section_type, title, content, font_family, font_size, sort_order, is_visible, created_at, updated_at
            "#
        )
        .bind(block_id)
        .bind(page_id)
        .bind(req.spa_section_id)
        .bind(section_key)
        .bind(req.block_type.as_str())
        .bind(&req.title)
        .bind(&content_json)
        .bind(font_family.as_str())
        .bind(font_size.as_str())
        .bind(sort_order)
        .bind(is_visible)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        for (idx, m) in media_dtos.iter().enumerate() {
            let media_sort = (idx as i32 + 1) * 10;
            let res = sqlx::query(
                "INSERT INTO section_media (id, section_id, media_asset_id, usage_type, sort_order) VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(Uuid::new_v4())
            .bind(block_id)
            .bind(m.id)
            .bind(DEFAULT_MEDIA_USAGE_TYPE)
            .bind(media_sort)
            .execute(&mut *tx)
            .await;

            if let Err(e) = res {
                let _ = tx.rollback().await;
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.code().as_deref() == Some("23505") {
                        return Err(AppError::ResourceConflict(
                            "Duplicate media asset attachment on content block".to_string(),
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
            media: media_dtos,
            font_family,
            font_size,
            sort_order: row.sort_order,
            is_visible: row.is_visible,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn list_blocks(
        &self,
        auth: &AuthenticatedUser,
        spa_section_id: Option<Uuid>,
    ) -> AppResult<Vec<AdminContentBlockDto>> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }
        self.list_content_blocks(spa_section_id).await
    }

    pub async fn get_block(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminContentBlockDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }
        self.get_content_block_by_id(id).await
    }

    pub async fn list_content_blocks(
        &self,
        spa_section_id: Option<Uuid>,
    ) -> AppResult<Vec<AdminContentBlockDto>> {
        let page_id = self.get_home_page_id().await?;

        let rows = if let Some(sec_id) = spa_section_id {
            // Verify section exists for home page
            let sec_exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM spa_sections WHERE id = $1 AND page_id = $2 AND deleted_at IS NULL",
            )
            .bind(sec_id)
            .bind(page_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            if sec_exists.is_none() {
                return Err(AppError::NotFound("SPA Section not found".to_string()));
            }

            sqlx::query_as::<_, JoinedAdminBlockRow>(
                r#"
                SELECT
                    s.id,
                    s.spa_section_id,
                    ss.section_key,
                    ss.title AS section_title,
                    s.section_type,
                    s.title,
                    s.content,
                    s.font_family,
                    s.font_size,
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
                    m.alt_text,
                    m.youtube_url,
                    m.thumbnail_url,
                    sm.sort_order AS media_sort_order
                FROM sections s
                JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
                JOIN pages p ON p.id = s.page_id
                LEFT JOIN section_media sm ON s.id = sm.section_id
                LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
                WHERE s.page_id = $1 AND s.spa_section_id = $2 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
                ORDER BY s.sort_order ASC, s.id ASC, sm.sort_order ASC, sm.created_at ASC, sm.id ASC
                "#
            )
            .bind(page_id)
            .bind(sec_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
        } else {
            sqlx::query_as::<_, JoinedAdminBlockRow>(
                r#"
                SELECT
                    s.id,
                    s.spa_section_id,
                    ss.section_key,
                    ss.title AS section_title,
                    s.section_type,
                    s.title,
                    s.content,
                    s.font_family,
                    s.font_size,
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
                    m.alt_text,
                    m.youtube_url,
                    m.thumbnail_url,
                    sm.sort_order AS media_sort_order
                FROM sections s
                JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
                JOIN pages p ON p.id = s.page_id
                LEFT JOIN section_media sm ON s.id = sm.section_id
                LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
                WHERE s.page_id = $1 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
                ORDER BY ss.sort_order ASC, s.sort_order ASC, s.id ASC, sm.sort_order ASC, sm.created_at ASC, sm.id ASC
                "#
            )
            .bind(page_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
        };

        Ok(Self::map_rows_to_dtos(rows))
    }

    fn map_rows_to_dtos(rows: Vec<JoinedAdminBlockRow>) -> Vec<AdminContentBlockDto> {
        let mut dtos: Vec<AdminContentBlockDto> = Vec::new();
        let mut block_index: HashMap<Uuid, usize> = HashMap::new();

        for row in rows {
            let entry_idx = match block_index.get(&row.id) {
                Some(&idx) => idx,
                None => {
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
                    let font_family =
                        FontFamily::parse(&row.font_family).unwrap_or(FontFamily::Sans);
                    let font_size = FontSize::parse(&row.font_size).unwrap_or(FontSize::Md);

                    let idx = dtos.len();
                    block_index.insert(row.id, idx);
                    dtos.push(AdminContentBlockDto {
                        id: row.id,
                        spa_section_id: row.spa_section_id,
                        section_key: row.section_key,
                        section_title: row.section_title,
                        block_type,
                        title: row.title,
                        text,
                        media: Vec::new(),
                        font_family,
                        font_size,
                        sort_order: row.sort_order,
                        is_visible: row.is_visible,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    });
                    idx
                }
            };

            if let Some(mid) = row.media_id {
                let url = if row.storage_provider.as_deref() == Some("youtube") {
                    row.youtube_url.clone()
                } else {
                    row.stored_filename
                        .as_ref()
                        .map(|f| format!("/uploads/{}", f))
                };

                dtos[entry_idx].media.push(BlockAttachedMediaDto {
                    id: mid,
                    media_type: row.media_type.unwrap_or_default(),
                    storage_provider: row.storage_provider.unwrap_or_default(),
                    original_filename: row.original_filename,
                    stored_filename: row.stored_filename,
                    mime_type: row.mime_type,
                    file_size: row.file_size,
                    youtube_url: row.youtube_url,
                    thumbnail_url: row.thumbnail_url,
                    url,
                    alt_text: row.alt_text,
                    sort_order: row.media_sort_order.unwrap_or(0),
                });
            }
        }

        dtos
    }

    pub async fn get_content_block_by_id(&self, id: Uuid) -> AppResult<AdminContentBlockDto> {
        let page_id = self.get_home_page_id().await?;

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
                s.font_family,
                s.font_size,
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
                m.alt_text,
                m.youtube_url,
                m.thumbnail_url,
                sm.sort_order AS media_sort_order
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            JOIN pages p ON p.id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.id = $1 AND s.page_id = $2 AND p.slug = 'home' AND s.deleted_at IS NULL AND ss.deleted_at IS NULL
            ORDER BY sm.sort_order ASC, sm.created_at ASC, sm.id ASC
            "#
        )
        .bind(id)
        .bind(page_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if rows.is_empty() {
            return Err(AppError::NotFound("Content block not found".to_string()));
        }

        let mut dtos = Self::map_rows_to_dtos(rows);
        dtos.pop()
            .ok_or_else(|| AppError::NotFound("Content block not found".to_string()))
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

        // 1. Transactionally load and lock current home block rows inside tx
        let current_rows = Self::get_home_block_for_update_tx(&mut tx, id).await?;
        let first_row = &current_rows[0];

        let current_block_type = ContentBlockType::parse(&first_row.section_type)
            .ok_or_else(|| AppError::Internal("Invalid block type".to_string()))?;

        let current_text = first_row
            .content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let current_media_ids: Vec<Uuid> = current_rows.iter().filter_map(|r| r.media_id).collect();

        // Section Move Logic
        let (target_spa_section_id, new_sec_key, new_sec_title, target_sort_order) =
            if let Some(target_sec_id) = req.spa_section_id {
                if target_sec_id != first_row.spa_section_id {
                    let (sec1, sec2) = if first_row.spa_section_id < target_sec_id {
                        (first_row.spa_section_id, target_sec_id)
                    } else {
                        (target_sec_id, first_row.spa_section_id)
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

                    let (target_key, target_title) =
                        Self::validate_and_lock_spa_section_tx(&mut tx, target_sec_id).await?;

                    let new_order =
                        Self::get_next_sort_order_for_section_tx(&mut tx, target_sec_id).await?;

                    (target_sec_id, target_key, target_title, new_order)
                } else {
                    (
                        first_row.spa_section_id,
                        first_row.section_key.clone(),
                        first_row.section_title.clone(),
                        first_row.sort_order,
                    )
                }
            } else {
                (
                    first_row.spa_section_id,
                    first_row.section_key.clone(),
                    first_row.section_title.clone(),
                    first_row.sort_order,
                )
            };

        let new_block_type = req.block_type.unwrap_or(current_block_type);
        let new_title = match req.title {
            Some(t) => Some(t),
            None => first_row.title.clone(),
        };
        let new_text = req.text.unwrap_or(current_text);
        let new_is_visible = req.is_visible.unwrap_or(first_row.is_visible);

        let new_font_family = req
            .font_family
            .or_else(|| FontFamily::parse(&first_row.font_family))
            .unwrap_or(FontFamily::Sans);

        let new_font_size = req
            .font_size
            .or_else(|| FontSize::parse(&first_row.font_size))
            .unwrap_or(FontSize::Md);

        self.validate_text_and_title(new_title.as_deref(), &new_text)?;

        // Media resolution: check media_ids or legacy media_id
        let media_mutation_requested =
            req.media_ids.is_some() || req.media_id.is_some() || req.block_type.is_some();

        let target_media_ids = if let Some(opt_ids) = req.media_ids {
            opt_ids.unwrap_or_default()
        } else if let Some(opt_mid) = req.media_id {
            match opt_mid {
                Some(mid) => vec![mid],
                None => Vec::new(),
            }
        } else {
            current_media_ids.clone()
        };

        let media_dtos =
            Self::validate_media_for_type_tx(&mut tx, new_block_type, &target_media_ids).await?;
        let content_json = serde_json::json!({ "text": new_text });

        let updated_row = sqlx::query_as::<_, SectionRow>(
            r#"
            UPDATE sections
            SET spa_section_id = $1, section_type = $2, title = $3, content = $4, font_family = $5, font_size = $6, sort_order = $7, is_visible = $8, updated_at = CURRENT_TIMESTAMP, updated_by = $9
            WHERE id = $10 AND deleted_at IS NULL
            RETURNING id, spa_section_id, section_type, title, content, font_family, font_size, sort_order, is_visible, created_at, updated_at
            "#
        )
        .bind(target_spa_section_id)
        .bind(new_block_type.as_str())
        .bind(&new_title)
        .bind(&content_json)
        .bind(new_font_family.as_str())
        .bind(new_font_size.as_str())
        .bind(target_sort_order)
        .bind(new_is_visible)
        .bind(auth.id)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Mutate section_media ONLY if media or block_type was explicitly updated
        if media_mutation_requested {
            let media_changed =
                target_media_ids != current_media_ids || new_block_type != current_block_type;

            if media_changed {
                sqlx::query("DELETE FROM section_media WHERE section_id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

                for (idx, m) in media_dtos.iter().enumerate() {
                    let media_sort = (idx as i32 + 1) * 10;
                    let res = sqlx::query(
                        "INSERT INTO section_media (id, section_id, media_asset_id, usage_type, sort_order) VALUES ($1, $2, $3, $4, $5)"
                    )
                    .bind(Uuid::new_v4())
                    .bind(id)
                    .bind(m.id)
                    .bind(DEFAULT_MEDIA_USAGE_TYPE)
                    .bind(media_sort)
                    .execute(&mut *tx)
                    .await;

                    if let Err(e) = res {
                        let _ = tx.rollback().await;
                        if let sqlx::Error::Database(ref db_err) = e {
                            if db_err.code().as_deref() == Some("23505") {
                                return Err(AppError::ResourceConflict(
                                    "Duplicate media asset attachment on content block".to_string(),
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
            media: media_dtos,
            font_family: new_font_family,
            font_size: new_font_size,
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

        // 1. Transactionally lock row verifying home page ownership
        let current_rows = Self::get_home_block_for_update_tx(&mut tx, id).await?;
        let first_row = &current_rows[0];

        // 2. Soft-delete row
        sqlx::query(
            "UPDATE sections SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, updated_by = $1 WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(auth.id)
        .bind(first_row.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 3. Delete attachments
        sqlx::query("DELETE FROM section_media WHERE section_id = $1")
            .bind(first_row.id)
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

        // Fetch active block IDs belonging to this section in current sort order
        let active_rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM sections WHERE spa_section_id = $1 AND deleted_at IS NULL ORDER BY sort_order ASC, id ASC",
        )
        .bind(spa_section_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let active_ids: Vec<Uuid> = active_rows.into_iter().map(|r| r.0).collect();
        let db_ids_set: HashSet<Uuid> = active_ids.iter().cloned().collect();

        // If DB has 0 blocks and req.items is empty -> success
        if db_ids_set.is_empty() && req.items.is_empty() {
            tx.commit()
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            return Ok(());
        }

        if !db_ids_set.is_empty() && req.items.is_empty() {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "items".to_string(),
                message: "Cannot reorder: items array is empty while section has active blocks"
                    .to_string(),
            }]));
        }

        // Check for duplicate block IDs in request and verify all items belong to this section
        let mut seen_ids = HashSet::new();
        for item in &req.items {
            if !db_ids_set.contains(&item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!(
                        "Referenced ContentBlock ID {} does not belong to this section or is deleted",
                        item.id
                    ),
                }]));
            }

            if !seen_ids.insert(item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("Duplicate block ID in reorder request: {}", item.id),
                }]));
            }
        }

        // Robust sort order determination:
        // Sort items by (sort_order, index_in_request). Does NOT fail on duplicate sort_orders or gaps!
        let mut indexed_req: Vec<(usize, &ReorderContentBlockItem)> =
            req.items.iter().enumerate().collect();

        indexed_req.sort_by(|(idx_a, item_a), (idx_b, item_b)| {
            let order_a = item_a.sort_order.unwrap_or(0);
            let order_b = item_b.sort_order.unwrap_or(0);
            order_a.cmp(&order_b).then_with(|| idx_a.cmp(idx_b))
        });

        let reordered_target_ids: Vec<Uuid> =
            indexed_req.into_iter().map(|(_, it)| it.id).collect();

        // Merge with any active blocks in this section that were not explicitly in req.items
        let mut final_ordered_ids = Vec::with_capacity(active_ids.len());
        let reordered_set: HashSet<Uuid> = reordered_target_ids.iter().cloned().collect();

        if reordered_target_ids.len() == active_ids.len() {
            final_ordered_ids = reordered_target_ids;
        } else {
            final_ordered_ids.extend(reordered_target_ids);
            for id in &active_ids {
                if !reordered_set.contains(id) {
                    final_ordered_ids.push(*id);
                }
            }
        }

        // Two-phase reorder: Phase 1 assign temporary negative order offset to prevent transient collision
        for (idx, id) in final_ordered_ids.iter().enumerate() {
            let temp_order = -100000 - (idx as i32);
            sqlx::query(
                "UPDATE sections SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 AND spa_section_id = $3 AND deleted_at IS NULL",
            )
            .bind(temp_order)
            .bind(id)
            .bind(spa_section_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Phase 2: assign final canonical target sort order
        for (idx, id) in final_ordered_ids.iter().enumerate() {
            let final_order = (idx as i32 + 1) * 10;
            sqlx::query(
                "UPDATE sections SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 AND spa_section_id = $3 AND deleted_at IS NULL",
            )
            .bind(final_order)
            .bind(id)
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
