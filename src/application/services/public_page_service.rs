use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use sqlx::PgPool;

use crate::application::dto::{
    PublicContentBlockDto, PublicMediaDto, PublicPageDto, PublicPageResponse, PublicSpaSectionDto,
};
use crate::domain::locale::Locale;
use crate::domain::media::YoutubeUrlParser;
use crate::domain::sections::{ContentBlockType, FontFamily, FontSize};
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
    /// (1. Home Page Metadata, 2. Visible SpaSections with translations, 3. Visible ContentBlocks + Media with translations).
    /// Grouping is performed in-memory in O(N) time without database queries inside loops (NO N+1).
    pub async fn get_home_page(&self, locale: Locale) -> AppResult<PublicPageResponse> {
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
            #[allow(dead_code)]
            navigation_label: String,
            sort_order: i32,
            req_name: Option<String>,
            en_name: Option<String>,
            req_nav: Option<String>,
            en_nav: Option<String>,
        }

        #[derive(sqlx::FromRow)]
        struct JoinedBlockRow {
            block_id: Uuid,
            spa_section_id: Uuid,
            section_type: String,
            title: Option<String>,
            content: serde_json::Value,
            font_family: String,
            font_size: String,
            sort_order: i32,
            media_id: Option<Uuid>,
            media_type: Option<String>,
            storage_key: Option<String>,
            mime_type: Option<String>,
            alt_text: Option<String>,
            youtube_video_id: Option<String>,
            #[allow(dead_code)]
            media_sort_order: Option<i32>,
            req_title: Option<String>,
            req_text: Option<String>,
            en_title: Option<String>,
            en_text: Option<String>,
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

        // Query 2: Bounded query for visible non-deleted SpaSections with translations ordered deterministically
        let section_rows = sqlx::query_as::<_, SectionRow>(
            r#"
            SELECT
                s.id,
                s.section_key AS key,
                s.title,
                s.navigation_label,
                s.sort_order,
                st_req.name AS req_name,
                st_en.name AS en_name,
                st_req.navigation_label AS req_nav,
                st_en.navigation_label AS en_nav
            FROM spa_sections s
            LEFT JOIN spa_section_translations st_req
                ON st_req.spa_section_id = s.id AND st_req.locale = $2
            LEFT JOIN spa_section_translations st_en
                ON st_en.spa_section_id = s.id AND st_en.locale = 'en'
            WHERE s.page_id = $1 AND s.deleted_at IS NULL AND s.is_visible = TRUE
            ORDER BY s.sort_order ASC, s.id ASC
            "#,
        )
        .bind(page_row.id)
        .bind(locale.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut sections: Vec<PublicSpaSectionDto> = Vec::with_capacity(section_rows.len());
        let mut section_index_map: HashMap<Uuid, usize> =
            HashMap::with_capacity(section_rows.len());

        for (idx, sec_row) in section_rows.into_iter().enumerate() {
            section_index_map.insert(sec_row.id, idx);

            let effective_name = sec_row
                .req_name
                .as_ref()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    sec_row
                        .en_name
                        .as_ref()
                        .map(|v| v.trim())
                        .filter(|v| !v.is_empty())
                })
                .unwrap_or(sec_row.title.as_str())
                .to_string();

            let effective_nav = sec_row
                .req_nav
                .as_ref()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    sec_row
                        .en_nav
                        .as_ref()
                        .map(|v| v.trim())
                        .filter(|v| !v.is_empty())
                })
                .or_else(|| {
                    sec_row
                        .req_name
                        .as_ref()
                        .map(|v| v.trim())
                        .filter(|v| !v.is_empty())
                })
                .or_else(|| {
                    sec_row
                        .en_name
                        .as_ref()
                        .map(|v| v.trim())
                        .filter(|v| !v.is_empty())
                })
                .or_else(|| {
                    let trimmed = sec_row.navigation_label.trim();
                    if !trimmed.is_empty() {
                        Some(trimmed)
                    } else {
                        None
                    }
                })
                .unwrap_or(sec_row.title.as_str())
                .to_string();

            sections.push(PublicSpaSectionDto {
                id: sec_row.id,
                key: sec_row.key,
                title: effective_name,
                navigation_label: effective_nav,
                sort_order: sec_row.sort_order,
                blocks: Vec::new(),
            });
        }

        // Query 3: Bounded query for visible non-deleted ContentBlocks & Media with translations
        let joined_rows = sqlx::query_as::<_, JoinedBlockRow>(
            r#"
            SELECT
                s.id AS block_id,
                s.spa_section_id,
                s.section_type,
                s.title,
                s.content,
                s.font_family,
                s.font_size,
                s.sort_order,
                m.id AS media_id,
                m.media_type,
                m.storage_key,
                m.mime_type,
                m.alt_text,
                m.youtube_video_id,
                sm.sort_order AS media_sort_order,
                ct_req.title AS req_title,
                ct_req.text AS req_text,
                ct_en.title AS en_title,
                ct_en.text AS en_text
            FROM sections s
            JOIN spa_sections ss ON ss.id = s.spa_section_id AND ss.page_id = s.page_id
            LEFT JOIN content_block_translations ct_req
                ON ct_req.content_block_id = s.id AND ct_req.locale = $2
            LEFT JOIN content_block_translations ct_en
                ON ct_en.content_block_id = s.id AND ct_en.locale = 'en'
            LEFT JOIN section_media sm ON s.id = sm.section_id
            LEFT JOIN media_assets m ON sm.media_asset_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE s.page_id = $1
              AND ss.deleted_at IS NULL
              AND ss.is_visible = TRUE
              AND s.deleted_at IS NULL
              AND s.is_visible = TRUE
            ORDER BY ss.sort_order ASC, ss.id ASC, s.sort_order ASC, s.id ASC, sm.sort_order ASC, sm.created_at ASC, sm.id ASC
            "#
        )
        .bind(page_row.id)
        .bind(locale.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // In-memory grouping: block_id -> (sec_idx, block_idx)
        let mut block_index_map: HashMap<Uuid, (usize, usize)> = HashMap::new();

        for row in joined_rows {
            let sec_idx = match section_index_map.get(&row.spa_section_id) {
                Some(&idx) => idx,
                None => continue,
            };

            let block_location = match block_index_map.get(&row.block_id) {
                Some(&loc) => loc,
                None => {
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

                    let fallback_text = row
                        .content
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let text = if let Some(ref t) = row.req_text {
                        if !t.trim().is_empty() {
                            t.clone()
                        } else if let Some(ref en_t) = row.en_text {
                            if !en_t.trim().is_empty() {
                                en_t.clone()
                            } else {
                                fallback_text
                            }
                        } else {
                            fallback_text
                        }
                    } else if let Some(ref en_t) = row.en_text {
                        if !en_t.trim().is_empty() {
                            en_t.clone()
                        } else {
                            fallback_text
                        }
                    } else {
                        fallback_text
                    };

                    let title = row
                        .req_title
                        .filter(|t| !t.trim().is_empty())
                        .or_else(|| row.en_title.filter(|t| !t.trim().is_empty()))
                        .or_else(|| row.title.filter(|t| !t.trim().is_empty()));

                    let font_family =
                        FontFamily::parse(&row.font_family).unwrap_or(FontFamily::Sans);
                    let font_size = FontSize::parse(&row.font_size).unwrap_or(FontSize::Md);

                    let b_idx = sections[sec_idx].blocks.len();
                    sections[sec_idx].blocks.push(PublicContentBlockDto {
                        id: row.block_id,
                        block_type,
                        title,
                        text,
                        media: Vec::new(),
                        font_family,
                        font_size,
                        sort_order: row.sort_order,
                    });
                    block_index_map.insert(row.block_id, (sec_idx, b_idx));
                    (sec_idx, b_idx)
                }
            };

            if let Some(mid) = row.media_id {
                let m_type = row.media_type.as_deref().unwrap_or_default();
                let public_media = match m_type {
                    "image" => {
                        let key = match row.storage_key.as_deref() {
                            Some(k) if !k.trim().is_empty() => k.trim(),
                            _ => {
                                tracing::warn!(
                                    "Skipping malformed image media {}: missing or empty storage key",
                                    mid
                                );
                                continue;
                            }
                        };
                        let url = self.storage.get_public_url(key);
                        PublicMediaDto::Image {
                            id: mid,
                            url,
                            alt_text: row.alt_text,
                        }
                    }
                    "video" => {
                        let key = match row.storage_key.as_deref() {
                            Some(k) if !k.trim().is_empty() => k.trim(),
                            _ => {
                                tracing::warn!(
                                    "Skipping malformed video media {}: missing or empty storage key",
                                    mid
                                );
                                continue;
                            }
                        };
                        let url = self.storage.get_public_url(key);
                        PublicMediaDto::Video {
                            id: mid,
                            url,
                            mime_type: row.mime_type,
                        }
                    }
                    "youtube" => {
                        let yid = match row.youtube_video_id {
                            Some(ref id) if !id.trim().is_empty() => id.trim().to_string(),
                            _ => {
                                tracing::warn!(
                                    "Skipping malformed youtube media {}: missing video ID",
                                    mid
                                );
                                continue;
                            }
                        };
                        let embed_url = YoutubeUrlParser::build_embed_url(&yid);
                        let thumbnail_url = YoutubeUrlParser::build_thumbnail_url(&yid);
                        PublicMediaDto::Youtube {
                            id: mid,
                            youtube_video_id: yid,
                            embed_url,
                            thumbnail_url,
                        }
                    }
                    _ => continue,
                };

                sections[block_location.0].blocks[block_location.1]
                    .media
                    .push(public_media);
            }
        }

        // Post-validation filter: eliminate blocks that violate type constraints
        for sec in &mut sections {
            sec.blocks.retain(|block| match block.block_type {
                ContentBlockType::Text => {
                    if !block.media.is_empty() {
                        tracing::warn!(
                            "Filtered malformed text block {}: has media attached",
                            block.id
                        );
                        false
                    } else {
                        true
                    }
                }
                ContentBlockType::TextImage => {
                    if block.media.is_empty() {
                        tracing::warn!(
                            "Filtered malformed text_image block {}: missing image media",
                            block.id
                        );
                        false
                    } else {
                        true
                    }
                }
                ContentBlockType::TextVideo => {
                    if block.media.is_empty() {
                        tracing::warn!(
                            "Filtered malformed text_video block {}: missing video media",
                            block.id
                        );
                        false
                    } else {
                        true
                    }
                }
                ContentBlockType::TextYoutube => {
                    if block.media.is_empty() {
                        tracing::warn!(
                            "Filtered malformed text_youtube block {}: missing youtube media",
                            block.id
                        );
                        false
                    } else {
                        true
                    }
                }
            });
        }

        // Query 4: Bounded query for visible non-deleted testimonials ordered deterministically
        let testimonial_repo =
            crate::infrastructure::repositories::testimonial_repository::TestimonialRepository::new(
                &self.pool,
            );
        let testimonial_rows = testimonial_repo.list_public_visible().await?;

        let testimonials: Vec<crate::application::dto::PublicTestimonialDto> = testimonial_rows
            .into_iter()
            .map(|row| {
                let avatar = if let (Some(_), Some(key)) = (row.media_id, row.media_storage_key) {
                    let trimmed = key.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(crate::application::dto::PublicTestimonialAvatarDto {
                            url: self.storage.get_public_url(trimmed),
                        })
                    }
                } else {
                    None
                };

                crate::application::dto::PublicTestimonialDto {
                    id: row.id,
                    author_name: row.author_name,
                    author_role: row.author_role,
                    text: row.text,
                    avatar,
                    sort_order: row.sort_order,
                }
            })
            .collect();

        Ok(PublicPageResponse {
            page: page_dto,
            sections,
            testimonials,
        })
    }
}
