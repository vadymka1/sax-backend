use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminTestimonialAvatarDto, AdminTestimonialDto, CreateTestimonialRequest,
    ReorderTestimonialsRequest, UpdateTestimonialRequest,
};
use crate::domain::testimonials::{validate_author_name, validate_author_role, validate_text};
use crate::domain::users::Role;
use crate::infrastructure::repositories::testimonial_repository::{
    TestimonialRepository, TestimonialRowWithMedia,
};
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub struct TestimonialService<'a> {
    pool: &'a PgPool,
    storage: Arc<dyn StorageProvider>,
}

impl<'a> TestimonialService<'a> {
    pub fn new(pool: &'a PgPool, storage: Arc<dyn StorageProvider>) -> Self {
        Self { pool, storage }
    }

    fn check_admin_access(auth: &AuthenticatedUser) -> AppResult<()> {
        match auth.role {
            Role::Admin | Role::SuperAdmin => Ok(()),
            _ => Err(AppError::Forbidden),
        }
    }

    async fn validate_avatar_media(&self, media_id: Uuid) -> AppResult<()> {
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
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "avatar_media_id".to_string(),
                        message: format!("Media asset is not active: {}", media_id),
                    }]));
                }
                if media_type != "image" {
                    return Err(AppError::ValidationError(vec![ApiErrorDetails {
                        field: "avatar_media_id".to_string(),
                        message: format!(
                            "Avatar media asset must be an image, got: {}",
                            media_type
                        ),
                    }]));
                }
                Ok(())
            }
            None => Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "avatar_media_id".to_string(),
                message: format!("Media asset not found: {}", media_id),
            }])),
        }
    }

    fn map_row_to_admin_dto(&self, row: TestimonialRowWithMedia) -> AdminTestimonialDto {
        let avatar = if let (Some(mid), Some(key)) = (row.media_id, row.media_storage_key) {
            Some(AdminTestimonialAvatarDto {
                id: mid,
                url: self.storage.get_public_url(&key),
                alt_text: row.media_alt_text,
            })
        } else {
            None
        };

        AdminTestimonialDto {
            id: row.id,
            author_name: row.author_name,
            author_role: row.author_role,
            text: row.text,
            avatar,
            sort_order: row.sort_order,
            is_visible: row.is_visible,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    pub async fn list_testimonials(
        &self,
        auth: &AuthenticatedUser,
    ) -> AppResult<Vec<AdminTestimonialDto>> {
        Self::check_admin_access(auth)?;
        let repo = TestimonialRepository::new(self.pool);
        let rows = repo.list_active().await?;
        Ok(rows
            .into_iter()
            .map(|r| self.map_row_to_admin_dto(r))
            .collect())
    }

    pub async fn get_testimonial(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminTestimonialDto> {
        Self::check_admin_access(auth)?;
        let repo = TestimonialRepository::new(self.pool);
        let row = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Testimonial not found".to_string()))?;
        Ok(self.map_row_to_admin_dto(row))
    }

    pub async fn create_testimonial(
        &self,
        auth: &AuthenticatedUser,
        req: CreateTestimonialRequest,
    ) -> AppResult<AdminTestimonialDto> {
        Self::check_admin_access(auth)?;

        let mut errors = Vec::new();

        let clean_author_name = match validate_author_name(&req.author_name) {
            Ok(name) => Some(name),
            Err(msg) => {
                errors.push(ApiErrorDetails {
                    field: "author_name".to_string(),
                    message: msg,
                });
                None
            }
        };

        let clean_author_role = match validate_author_role(req.author_role.as_deref()) {
            Ok(role) => role,
            Err(msg) => {
                errors.push(ApiErrorDetails {
                    field: "author_role".to_string(),
                    message: msg,
                });
                None
            }
        };

        let clean_text = match validate_text(&req.text) {
            Ok(txt) => Some(txt),
            Err(msg) => {
                errors.push(ApiErrorDetails {
                    field: "text".to_string(),
                    message: msg,
                });
                None
            }
        };

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        if let Some(media_id) = req.avatar_media_id {
            self.validate_avatar_media(media_id).await?;
        }

        let is_visible = req.is_visible.unwrap_or(true);
        let repo = TestimonialRepository::new(self.pool);
        let row = repo
            .create(
                &clean_author_name.unwrap(),
                clean_author_role.as_deref(),
                &clean_text.unwrap(),
                req.avatar_media_id,
                is_visible,
            )
            .await?;

        Ok(self.map_row_to_admin_dto(row))
    }

    pub async fn update_testimonial(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
        req: UpdateTestimonialRequest,
    ) -> AppResult<AdminTestimonialDto> {
        Self::check_admin_access(auth)?;

        let mut errors = Vec::new();

        let clean_author_name = if let Some(ref name) = req.author_name {
            match validate_author_name(name) {
                Ok(clean) => Some(clean),
                Err(msg) => {
                    errors.push(ApiErrorDetails {
                        field: "author_name".to_string(),
                        message: msg,
                    });
                    None
                }
            }
        } else {
            None
        };

        let clean_author_role = if let Some(ref opt_role) = req.author_role {
            match validate_author_role(opt_role.as_deref()) {
                Ok(clean) => Some(clean),
                Err(msg) => {
                    errors.push(ApiErrorDetails {
                        field: "author_role".to_string(),
                        message: msg,
                    });
                    None
                }
            }
        } else {
            None
        };

        let clean_text = if let Some(ref txt) = req.text {
            match validate_text(txt) {
                Ok(clean) => Some(clean),
                Err(msg) => {
                    errors.push(ApiErrorDetails {
                        field: "text".to_string(),
                        message: msg,
                    });
                    None
                }
            }
        } else {
            None
        };

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        // Validate avatar media if provided
        if let Some(Some(mid)) = req.avatar_media_id {
            self.validate_avatar_media(mid).await?;
        }

        let repo = TestimonialRepository::new(self.pool);
        let opt_role_ref = clean_author_role.as_ref().map(|opt| opt.as_deref());

        let updated_row = repo
            .update(
                id,
                clean_author_name.as_deref(),
                opt_role_ref,
                clean_text.as_deref(),
                req.avatar_media_id,
                req.is_visible,
            )
            .await?;

        let row =
            updated_row.ok_or_else(|| AppError::NotFound("Testimonial not found".to_string()))?;
        Ok(self.map_row_to_admin_dto(row))
    }

    pub async fn delete_testimonial(&self, auth: &AuthenticatedUser, id: Uuid) -> AppResult<()> {
        Self::check_admin_access(auth)?;
        let repo = TestimonialRepository::new(self.pool);
        let deleted = repo.soft_delete(id).await?;
        if !deleted {
            return Err(AppError::NotFound("Testimonial not found".to_string()));
        }
        Ok(())
    }

    pub async fn reorder_testimonials(
        &self,
        auth: &AuthenticatedUser,
        req: ReorderTestimonialsRequest,
    ) -> AppResult<()> {
        Self::check_admin_access(auth)?;
        let repo = TestimonialRepository::new(self.pool);
        repo.reorder_testimonials_tx(&req.items).await
    }
}
