use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminSpaSectionDto, CreateSpaSectionRequest, ReorderSpaSectionsRequest, UpdateSpaSectionRequest,
};
use crate::domain::sections::{generate_slug, validate_spa_section_key, SpaSection};
use crate::infrastructure::repositories::spa_section_repository::SpaSectionRepository;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub struct SpaSectionService<'a> {
    pool: &'a PgPool,
    repo: SpaSectionRepository<'a>,
}

impl<'a> SpaSectionService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            repo: SpaSectionRepository::new(pool),
        }
    }

    pub async fn get_home_page_id(&self) -> AppResult<Uuid> {
        let res: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
            .fetch_optional(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        res.map(|r| r.0)
            .ok_or_else(|| AppError::NotFound("Home page not found".to_string()))
    }

    pub async fn get_by_id(&self, id: Uuid) -> AppResult<SpaSection> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("SPA Section not found".to_string()))
    }

    pub async fn get_by_key(&self, page_id: Uuid, key: &str) -> AppResult<SpaSection> {
        if !validate_spa_section_key(key) {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "section_key".to_string(),
                message: "Invalid SPA section key format. Must be lowercase a-z, 0-9, or hyphens"
                    .to_string(),
            }]));
        }

        self.repo
            .find_by_key(page_id, key)
            .await?
            .ok_or_else(|| AppError::NotFound("SPA Section not found".to_string()))
    }

    pub async fn list_for_page(&self, page_id: Uuid) -> AppResult<Vec<SpaSection>> {
        self.repo.list_for_page(page_id).await
    }

    pub async fn validate_section_exists(&self, id: Uuid) -> AppResult<()> {
        if !self.repo.exists(id).await? {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "spa_section_id".to_string(),
                message: "Referenced SPA section does not exist".to_string(),
            }]));
        }
        Ok(())
    }

    // --- Admin CRUD Methods ---

    pub async fn list_admin_sections(
        &self,
        auth: &AuthenticatedUser,
    ) -> AppResult<Vec<AdminSpaSectionDto>> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let home_id = self.get_home_page_id().await?;
        self.repo.list_admin_for_page(home_id).await
    }

    pub async fn get_admin_section(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminSpaSectionDto> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let home_id = self.get_home_page_id().await?;
        self.repo
            .find_admin_by_id(home_id, id)
            .await?
            .ok_or_else(|| AppError::NotFound("SPA Section not found".to_string()))
    }

    pub async fn create_section(
        &self,
        auth: &AuthenticatedUser,
        req: CreateSpaSectionRequest,
    ) -> AppResult<AdminSpaSectionDto> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let mut errors = Vec::new();

        let (en_name, de_name) = if let Some(ref trans) = req.translations {
            let en_trimmed = trans.en.name.trim();
            if en_trimmed.is_empty() {
                errors.push(ApiErrorDetails {
                    field: "translations.en.name".to_string(),
                    message: "English name is required and cannot be empty".to_string(),
                });
            } else if en_trimmed.len() > 255 {
                errors.push(ApiErrorDetails {
                    field: "translations.en.name".to_string(),
                    message: "English name cannot exceed 255 characters".to_string(),
                });
            }

            let de_val = if let Some(ref de) = trans.de {
                let de_trimmed = de.name.trim();
                if de_trimmed.is_empty() {
                    errors.push(ApiErrorDetails {
                        field: "translations.de.name".to_string(),
                        message: "German name cannot be empty or whitespace".to_string(),
                    });
                    None
                } else if de_trimmed.len() > 255 {
                    errors.push(ApiErrorDetails {
                        field: "translations.de.name".to_string(),
                        message: "German name cannot exceed 255 characters".to_string(),
                    });
                    None
                } else {
                    Some(de_trimmed.to_string())
                }
            } else {
                None
            };

            (en_trimmed.to_string(), de_val)
        } else if !req.title.trim().is_empty() {
            let title_trimmed = req.title.trim();
            if title_trimmed.len() > 255 {
                errors.push(ApiErrorDetails {
                    field: "title".to_string(),
                    message: "Title cannot exceed 255 characters".to_string(),
                });
            }
            (title_trimmed.to_string(), None)
        } else {
            errors.push(ApiErrorDetails {
                field: "title".to_string(),
                message: "Title or translations is required".to_string(),
            });
            (String::new(), None)
        };

        let nav_label = match req.navigation_label {
            Some(ref l) => l.trim().to_string(),
            None => en_name.clone(),
        };

        if nav_label.is_empty() && errors.is_empty() {
            errors.push(ApiErrorDetails {
                field: "navigation_label".to_string(),
                message: "Navigation label cannot be empty".to_string(),
            });
        } else if nav_label.len() > 100 {
            errors.push(ApiErrorDetails {
                field: "navigation_label".to_string(),
                message: "Navigation label cannot exceed 100 characters".to_string(),
            });
        }

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        let home_id = self.get_home_page_id().await?;
        let base_key = generate_slug(&en_name);

        let created = self
            .repo
            .create_dynamic_for_page(home_id, &base_key, &en_name, &nav_label, de_name.as_deref())
            .await?;

        self.repo
            .find_admin_by_id(home_id, created.id)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to retrieve created section".to_string()))
    }

    pub async fn update_section(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
        req: UpdateSpaSectionRequest,
    ) -> AppResult<AdminSpaSectionDto> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let home_id = self.get_home_page_id().await?;

        // Verify section exists and belongs to home page
        let _existing = self
            .repo
            .find_admin_by_id(home_id, id)
            .await?
            .ok_or_else(|| AppError::NotFound("SPA Section not found".to_string()))?;

        let mut errors = Vec::new();

        let title_opt = if let Some(ref t) = req.title {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                errors.push(ApiErrorDetails {
                    field: "title".to_string(),
                    message: "Title cannot be empty".to_string(),
                });
            } else if trimmed.len() > 255 {
                errors.push(ApiErrorDetails {
                    field: "title".to_string(),
                    message: "Title cannot exceed 255 characters".to_string(),
                });
            }
            Some(trimmed)
        } else {
            None
        };

        let nav_label_opt = if let Some(ref n) = req.navigation_label {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                errors.push(ApiErrorDetails {
                    field: "navigation_label".to_string(),
                    message: "Navigation label cannot be empty".to_string(),
                });
            } else if trimmed.len() > 100 {
                errors.push(ApiErrorDetails {
                    field: "navigation_label".to_string(),
                    message: "Navigation label cannot exceed 100 characters".to_string(),
                });
            }
            Some(trimmed)
        } else {
            None
        };

        let (en_opt, de_opt) = if let Some(ref trans) = req.translations {
            let en_val = if let Some(ref en) = trans.en {
                let trimmed = en.name.trim();
                if trimmed.is_empty() {
                    errors.push(ApiErrorDetails {
                        field: "translations.en.name".to_string(),
                        message: "English name cannot be empty".to_string(),
                    });
                    None
                } else if trimmed.len() > 255 {
                    errors.push(ApiErrorDetails {
                        field: "translations.en.name".to_string(),
                        message: "English name cannot exceed 255 characters".to_string(),
                    });
                    None
                } else {
                    Some(trimmed)
                }
            } else {
                None
            };

            let de_val = if let Some(ref de) = trans.de {
                let trimmed = de.name.trim();
                if trimmed.is_empty() {
                    errors.push(ApiErrorDetails {
                        field: "translations.de.name".to_string(),
                        message: "German name cannot be empty or whitespace".to_string(),
                    });
                    None
                } else if trimmed.len() > 255 {
                    errors.push(ApiErrorDetails {
                        field: "translations.de.name".to_string(),
                        message: "German name cannot exceed 255 characters".to_string(),
                    });
                    None
                } else {
                    Some(trimmed)
                }
            } else {
                None
            };

            (en_val, de_val)
        } else {
            (None, None)
        };

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        self.repo
            .update_section(id, title_opt, nav_label_opt, req.is_visible, en_opt, de_opt)
            .await?;

        self.repo
            .find_admin_by_id(home_id, id)
            .await?
            .ok_or_else(|| AppError::NotFound("SPA Section not found after update".to_string()))
    }

    pub async fn delete_section(&self, auth: &AuthenticatedUser, id: Uuid) -> AppResult<()> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let home_id = self.get_home_page_id().await?;
        let deleted = self.repo.soft_delete_empty_for_page(home_id, id).await?;
        if !deleted {
            return Err(AppError::NotFound("SPA Section not found".to_string()));
        }

        Ok(())
    }

    pub async fn reorder_sections(
        &self,
        auth: &AuthenticatedUser,
        req: ReorderSpaSectionsRequest,
    ) -> AppResult<()> {
        if !auth.role.can_manage_content() {
            return Err(AppError::Forbidden);
        }

        let home_id = self.get_home_page_id().await?;
        self.repo.reorder_sections_tx(home_id, &req.items).await
    }
}
