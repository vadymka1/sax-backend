use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminMediaDto, AdminPageAppearanceDto, UpdatePageAppearanceRequest};
use crate::domain::pages::{BackgroundPosition, BackgroundSize, PageAppearanceSettings};
use crate::infrastructure::repositories::page_appearance_repository::PageAppearanceRepository;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub struct PageAppearanceService<'a> {
    pool: &'a PgPool,
    storage: Arc<dyn StorageProvider>,
    repo: PageAppearanceRepository<'a>,
}

impl<'a> PageAppearanceService<'a> {
    pub fn new(pool: &'a PgPool, storage: Arc<dyn StorageProvider>) -> Self {
        Self {
            pool,
            storage,
            repo: PageAppearanceRepository::new(pool),
        }
    }

    async fn get_home_page_id(&self) -> AppResult<Uuid> {
        let res: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM pages WHERE slug = 'home' AND deleted_at IS NULL")
                .fetch_optional(self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        match res {
            Some((id,)) => Ok(id),
            None => Err(AppError::NotFound("Home page not found".to_string())),
        }
    }

    pub async fn get_appearance(
        &self,
        auth: &AuthenticatedUser,
    ) -> AppResult<AdminPageAppearanceDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let page_id = self.get_home_page_id().await?;
        let row = self.repo.get_by_page_id(page_id).await?;

        match row {
            Some(r) => {
                let background_media = if let Some(media_id) = r.media_id {
                    let key = r.storage_key.unwrap_or_default();
                    let url = self.storage.get_public_url(&key);
                    Some(AdminMediaDto::Image {
                        id: media_id,
                        url,
                        original_filename: r.original_filename,
                        mime_type: r.mime_type.unwrap_or_else(|| "image/jpeg".to_string()),
                        file_size: r.file_size.unwrap_or(0),
                        alt_text: r.alt_text,
                        created_at: r.media_created_at.unwrap_or_else(Utc::now),
                    })
                } else {
                    None
                };

                Ok(AdminPageAppearanceDto {
                    background_media,
                    overlay_opacity: r.overlay_opacity,
                    background_position: BackgroundPosition::parse(&r.background_position)
                        .unwrap_or_default(),
                    background_size: BackgroundSize::parse(&r.background_size).unwrap_or_default(),
                })
            }
            None => Ok(AdminPageAppearanceDto {
                background_media: None,
                overlay_opacity: 0.35,
                background_position: BackgroundPosition::Center,
                background_size: BackgroundSize::Cover,
            }),
        }
    }

    pub async fn update_appearance(
        &self,
        auth: &AuthenticatedUser,
        req: UpdatePageAppearanceRequest,
    ) -> AppResult<AdminPageAppearanceDto> {
        if !auth.is_active || !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let mut errors = Vec::new();
        let mut media_not_found = None;

        // 1. Validate overlay opacity
        if let Some(opacity) = req.overlay_opacity {
            if opacity.is_nan() || opacity.is_infinite() || !(0.0..=1.0).contains(&opacity) {
                errors.push(ApiErrorDetails {
                    field: "overlay_opacity".to_string(),
                    message: "Overlay opacity must be between 0.0 and 1.0".to_string(),
                });
            }
        }

        // 2. Validate background media if provided
        if let Some(Some(media_id)) = req.background_media_id {
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT media_type, status FROM media_assets WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(media_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            match row {
                Some((media_type, status)) => {
                    if status != "active" {
                        errors.push(ApiErrorDetails {
                            field: "background_media_id".to_string(),
                            message: format!("Media asset is not active: {}", media_id),
                        });
                    }
                    if media_type != "image" {
                        errors.push(ApiErrorDetails {
                            field: "background_media_id".to_string(),
                            message: format!(
                                "Page background media must be an image, got: {}",
                                media_type
                            ),
                        });
                    }
                }
                None => {
                    media_not_found = Some(format!("Media asset not found: {}", media_id));
                }
            }
        }

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        if let Some(msg) = media_not_found {
            return Err(AppError::NotFound(msg));
        }

        let page_id = self.get_home_page_id().await?;
        let current = self
            .repo
            .get_raw_settings(page_id)
            .await?
            .unwrap_or_else(|| PageAppearanceSettings {
                page_id,
                ..Default::default()
            });

        let new_bg = match req.background_media_id {
            Some(val) => val,
            None => current.background_media_id,
        };

        let new_opacity = req.overlay_opacity.unwrap_or(current.overlay_opacity);
        let new_position = req
            .background_position
            .unwrap_or(current.background_position);
        let new_size = req.background_size.unwrap_or(current.background_size);

        self.repo
            .upsert(page_id, new_bg, new_opacity, new_position, new_size)
            .await?;

        self.get_appearance(auth).await
    }
}
