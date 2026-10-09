use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminMediaDto, AdminPageAppearanceDto, UpdatePageAppearanceRequest};
use crate::domain::pages::{
    validate_and_normalize_hex_color, BackgroundMode, BackgroundPosition, BackgroundSize,
    PageAppearanceSettings,
};
use crate::infrastructure::repositories::page_appearance_repository::{
    PageAppearanceRepository, UpsertAppearanceParams,
};
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
                    background_mode: BackgroundMode::parse(&r.background_mode).unwrap_or_default(),
                    background_color: r.background_color,
                    background_media,
                    overlay_opacity: r.overlay_opacity,
                    background_position: BackgroundPosition::parse(&r.background_position)
                        .unwrap_or_default(),
                    background_size: BackgroundSize::parse(&r.background_size).unwrap_or_default(),
                })
            }
            None => Ok(AdminPageAppearanceDto {
                background_mode: BackgroundMode::None,
                background_color: "#FFFFFF".to_string(),
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

        // 2. Validate background color
        let mut normalized_color = None;
        if let Some(ref color) = req.background_color {
            match validate_and_normalize_hex_color(color) {
                Ok(norm) => {
                    normalized_color = Some(norm);
                }
                Err(err_msg) => {
                    errors.push(ApiErrorDetails {
                        field: "background_color".to_string(),
                        message: err_msg,
                    });
                }
            }
        }

        // 3. Validate background media if provided as Some(Some(id))
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

        let final_mode = req.background_mode.unwrap_or(current.background_mode);
        let final_color = normalized_color.unwrap_or(current.background_color);
        let final_media_id = match req.background_media_id {
            Some(val) => val,
            None => current.background_media_id,
        };
        let final_opacity = req.overlay_opacity.unwrap_or(current.overlay_opacity);
        let final_position = req
            .background_position
            .unwrap_or(current.background_position);
        let final_size = req.background_size.unwrap_or(current.background_size);

        // 4. Final-state validation:
        // When final_mode is Image, a valid active image media asset MUST be assigned.
        if final_mode == BackgroundMode::Image {
            match final_media_id {
                Some(media_id) => {
                    // If media_id was not explicitly passed in this request, verify the currently assigned media is still active and image
                    if req.background_media_id.is_none() {
                        let row: Option<(String, String)> = sqlx::query_as(
                            "SELECT media_type, status FROM media_assets WHERE id = $1 AND deleted_at IS NULL",
                        )
                        .bind(media_id)
                        .fetch_optional(self.pool)
                        .await
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

                        match row {
                            Some((media_type, status)) => {
                                if status != "active" || media_type != "image" {
                                    errors.push(ApiErrorDetails {
                                        field: "background_mode".to_string(),
                                        message:
                                            "Assigned background media is not an active image asset"
                                                .to_string(),
                                    });
                                }
                            }
                            None => {
                                errors.push(ApiErrorDetails {
                                    field: "background_mode".to_string(),
                                    message: "Assigned background media asset not found"
                                        .to_string(),
                                });
                            }
                        }
                    }
                }
                None => {
                    errors.push(ApiErrorDetails {
                        field: "background_mode".to_string(),
                        message: "Background mode 'image' requires an active background image asset to be assigned".to_string(),
                    });
                }
            }
        }

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        self.repo
            .upsert(UpsertAppearanceParams {
                page_id,
                background_mode: final_mode,
                background_color: &final_color,
                background_media_id: final_media_id,
                overlay_opacity: final_opacity,
                background_position: final_position,
                background_size: final_size,
            })
            .await?;

        self.get_appearance(auth).await
    }
}
