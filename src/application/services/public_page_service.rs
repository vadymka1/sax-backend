use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use sqlx::PgPool;

use crate::application::dto::{
    PublicContentBlockDto, PublicMediaDto, PublicPageDto, PublicPageResponse, PublicSpaSectionDto,
};
use crate::domain::media::YoutubeUrlParser;
use crate::domain::sections::ContentBlockType;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{AppError, AppResult};

pub struct PublicPageService {
    pool: PgPool,
    storage: Arc<dyn StorageProvider>,
}

impl PublicPageService {
    pub fn new(pool: PgPool, storage: Arc<dyn StorageProvider>) -> Self {
        Self { pool, storage }
    }

    /// Fetches the complete grouped SPA structure for the home page.
    ///
    /// Architecture Note: Public SPA aggregation intentionally uses 3 bounded queries total
    /// (1. Home Page Metadata, 2. Visible SpaSections, 3. Visible ContentBlocks + Media).
    /// Grouping is performed in-memory in O(N) time without database queries inside loops (NO N+1).
    pub async fn get_home_page(&self) -> AppResult<PublicPageResponse> {
        #[derive(sqlx::FromRow)]
        struct PageRow {
            id: Uuid,
            slug: String,
            title: String,
            seo_title: Option<String>,
            seo_description: Option<String>,
            seo_keywords: Option<Vec<String>>,
        }

        #[derive(sqlx::FromRow)]
        struct SectionRow {
            id: Uuid,
            key: String,
            title: String,
            navigation_label: String,
            sort_order: i32,
        }

        #[derive(sqlx::FromRow)]
        struct JoinedBlockRow {
            block_id: Uuid,
            spa_section_id: Uuid,
            section_type: String,
            title: Option<String>,
            content: serde_json::Value,
            sort_order: i32,
            media_id: Option<Uuid>,
            media_type: Option<String>,
            storage_key: Option<String>,
            mime_type: Option<String>,
            alt_text: Option<String>,
            youtube_video_id: Option<String>,
        }

        // Query 1: Bounded query for home page metadata
        let page_row = sqlx::query_as::<_, PageRow>(
            "SELECT id, slug, title, seo_title, seo_description, seo_keywords FROM pages WHERE slug = 'home' AND deleted_at IS NULL"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Home page not found".to_string()))?;

        let page_dto = PublicPageDto {
            id: page_row.id,
            slug: page_row.slug,
            title: page_row.title,
            seo_title: page_row.seo_title,
            seo_description: page_row.seo_description,
            seo_keywords: page_row.seo_keywords,
        };

        // Query 2: Bounded query for visible non-deleted SpaSections ordered deterministically
        let section_rows = sqlx::query_as::<_, SectionRow>(
            r#"
            SELECT
                id,
                section_key AS key,
                title,
                navigation_label,
                sort_order
            FROM spa_sections
            WHERE page_id = $1 AND deleted_at IS NULL AND is_visible = TRUE
            ORDER BY sort_order ASC, id ASC
            "#,
        )
        .bind(page_row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut sections: Vec<PublicSpaSectionDto> = Vec::with_capacity(section_rows.len());
        let mut section_index_map: HashMap<Uuid, usize> =
            HashMap::with_capacity(section_rows.len());

        for (idx, sec_row) in section_rows.into_iter().enumerate() {
            section_index_map.insert(sec_row.id, idx);
            sections.push(PublicSpaSectionDto {
                id: sec_row.id,
                key: sec_row.key,
                title: sec_row.title,
                navigation_label: sec_row.navigation_label,
                sort_order: sec_row.sort_order,
                blocks: Vec::new(),
            });
        }

        // Query 3: Bounded query for visible non-deleted ContentBlocks & Media
        let joined_rows = sqlx::query_as::<_, JoinedBlockRow>(
            r#"
            SELECT
                s.id AS block_id,
                s.spa_section_id,
                s.section_type,
                s.title,
                s.content,
                s.sort_order,
                m.id AS media_id,
                m.media_type,
                m.storage_key,
                m.mime_type,
                m.alt_text,
                m.youtube_video_id
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.page_id = $1
              AND ss.deleted_at IS NULL
              AND ss.is_visible = TRUE
              AND s.deleted_at IS NULL
              AND s.is_visible = TRUE
            ORDER BY ss.sort_order ASC, ss.id ASC, s.sort_order ASC, s.id ASC
            "#
        )
        .bind(page_row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Efficient Rust O(N) grouping using section_index_map without N+1 queries
        for row in joined_rows {
            let sec_idx = match section_index_map.get(&row.spa_section_id) {
                Some(&idx) => idx,
                None => continue,
            };

            let block_type = match ContentBlockType::parse(&row.section_type) {
                Some(bt) => bt,
                None => {
                    tracing::warn!(
                        "Skipping invalid public content block {}: unsupported block type {}",
                        row.block_id,
                        row.section_type
                    );
                    continue;
                }
            };

            let text = row
                .content
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let media = match block_type {
                ContentBlockType::Text => {
                    if row.media_id.is_some() {
                        tracing::warn!(
                            "Skipping malformed text block {}: text block has media asset attached",
                            row.block_id
                        );
                        continue;
                    }
                    None
                }
                ContentBlockType::TextImage => {
                    let mid = match row.media_id {
                        Some(id) => id,
                        None => {
                            tracing::warn!(
                                "Skipping malformed text_image block {}: missing media asset",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    if row.media_type.as_deref() != Some("image") {
                        tracing::warn!(
                            "Skipping malformed text_image block {}: media asset is not an image",
                            row.block_id
                        );
                        continue;
                    }
                    let key = match row.storage_key.as_deref() {
                        Some(k) if !k.trim().is_empty() => k.trim(),
                        _ => {
                            tracing::warn!(
                                "Skipping malformed image block {}: missing or empty storage key",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    let url = self.storage.get_public_url(key);
                    Some(PublicMediaDto::Image {
                        id: mid,
                        url,
                        alt_text: row.alt_text,
                    })
                }
                ContentBlockType::TextVideo => {
                    let mid = match row.media_id {
                        Some(id) => id,
                        None => {
                            tracing::warn!(
                                "Skipping malformed text_video block {}: missing media asset",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    if row.media_type.as_deref() != Some("video") {
                        tracing::warn!(
                            "Skipping malformed text_video block {}: media asset is not a video",
                            row.block_id
                        );
                        continue;
                    }
                    let key = match row.storage_key.as_deref() {
                        Some(k) if !k.trim().is_empty() => k.trim(),
                        _ => {
                            tracing::warn!(
                                "Skipping malformed video block {}: missing or empty storage key",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    let url = self.storage.get_public_url(key);
                    Some(PublicMediaDto::Video {
                        id: mid,
                        url,
                        mime_type: row.mime_type,
                    })
                }
                ContentBlockType::TextYoutube => {
                    let mid = match row.media_id {
                        Some(id) => id,
                        None => {
                            tracing::warn!(
                                "Skipping malformed text_youtube block {}: missing media asset",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    if row.media_type.as_deref() != Some("youtube") {
                        tracing::warn!(
                            "Skipping malformed text_youtube block {}: media asset is not youtube",
                            row.block_id
                        );
                        continue;
                    }
                    let yid = match row.youtube_video_id {
                        Some(ref id) if !id.trim().is_empty() => id.trim().to_string(),
                        _ => {
                            tracing::warn!(
                                "Skipping malformed youtube block {}: missing or empty video ID",
                                row.block_id
                            );
                            continue;
                        }
                    };
                    let embed_url = YoutubeUrlParser::build_embed_url(&yid);
                    let thumbnail_url = YoutubeUrlParser::build_thumbnail_url(&yid);
                    Some(PublicMediaDto::Youtube {
                        id: mid,
                        youtube_video_id: yid,
                        embed_url,
                        thumbnail_url,
                    })
                }
            };

            sections[sec_idx].blocks.push(PublicContentBlockDto {
                id: row.block_id,
                block_type,
                title: row.title,
                text,
                media,
                sort_order: row.sort_order,
            });
        }

        Ok(PublicPageResponse {
            page: page_dto,
            sections,
        })
    }
}
