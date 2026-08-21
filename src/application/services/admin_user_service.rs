use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::ValidateEmail;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{CreateUserRequest, UpdateUserRequest, UserDto};
use crate::config::AppConfig;
use crate::domain::users::{Role, User};
use crate::infrastructure::auth::PasswordService;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::{PaginatedResponse, PaginationMeta};

pub struct AdminUserService<'a> {
    pool: &'a PgPool,
    config: &'a AppConfig,
}

impl<'a> AdminUserService<'a> {
    pub fn new(pool: &'a PgPool, config: &'a AppConfig) -> Self {
        Self { pool, config }
    }

    pub async fn list_users(
        &self,
        auth: &AuthenticatedUser,
        page: Option<i64>,
        page_size: Option<i64>,
    ) -> AppResult<PaginatedResponse<UserDto>> {
        if !auth.role.can_manage_users() {
            return Err(AppError::Forbidden);
        }

        let page_num = page.unwrap_or(1);
        if page_num < 1 {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "page".to_string(),
                message: "page must be greater than or equal to 1".to_string(),
            }]));
        }

        let size = page_size.unwrap_or(20);
        if !(1..=100).contains(&size) {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "page_size".to_string(),
                message: "page_size must be between 1 and 100".to_string(),
            }]));
        }

        let offset = (page_num - 1) * size;

        let users = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(size)
        .bind(offset)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let count_res: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let dtos = users
            .into_iter()
            .map(|u| UserDto {
                id: u.id,
                email: u.email,
                display_name: u.display_name,
                role: u.role,
                is_active: u.is_active,
                last_login_at: u.last_login_at,
                created_at: u.created_at,
                updated_at: u.updated_at,
            })
            .collect();

        Ok(PaginatedResponse {
            data: dtos,
            meta: PaginationMeta::new(page_num, size, count_res.0),
        })
    }

    pub async fn create_admin(
        &self,
        auth: &AuthenticatedUser,
        req: CreateUserRequest,
    ) -> AppResult<UserDto> {
        if !auth.role.can_manage_users() {
            return Err(AppError::Forbidden);
        }

        let mut errors = Vec::new();

        let email_norm = req.email.trim().to_lowercase();
        if email_norm.is_empty() {
            errors.push(ApiErrorDetails {
                field: "email".to_string(),
                message: "Email is required".to_string(),
            });
        } else if email_norm.len() > 255 || !ValidateEmail::validate_email(&email_norm) {
            errors.push(ApiErrorDetails {
                field: "email".to_string(),
                message: "Invalid email format".to_string(),
            });
        }

        let display_name = req.display_name.trim();
        if display_name.is_empty() {
            errors.push(ApiErrorDetails {
                field: "display_name".to_string(),
                message: "Display name cannot be empty".to_string(),
            });
        } else if display_name.len() < 2 || display_name.len() > 100 {
            errors.push(ApiErrorDetails {
                field: "display_name".to_string(),
                message: "Display name must be between 2 and 100 characters".to_string(),
            });
        }

        let password = req.password.trim();
        if password.is_empty() {
            errors.push(ApiErrorDetails {
                field: "password".to_string(),
                message: "Password cannot be empty".to_string(),
            });
        } else if req.password.len() < self.config.password_min_length {
            errors.push(ApiErrorDetails {
                field: "password".to_string(),
                message: format!(
                    "Password must be at least {} characters long",
                    self.config.password_min_length
                ),
            });
        } else if req.password.len() > 128 {
            errors.push(ApiErrorDetails {
                field: "password".to_string(),
                message: "Password exceeds maximum length".to_string(),
            });
        }

        let target_role = match req.role {
            Some(ref r) => match Role::parse(r) {
                Some(Role::Admin) => Role::Admin,
                Some(Role::SuperAdmin) => Role::SuperAdmin,
                _ => {
                    errors.push(ApiErrorDetails {
                        field: "role".to_string(),
                        message: "Invalid role value. Accepted roles are 'admin' and 'super_admin'"
                            .to_string(),
                    });
                    Role::Admin
                }
            },
            None => Role::Admin,
        };

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        let existing: (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
                .bind(&email_norm)
                .fetch_one(self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if existing.0 {
            return Err(AppError::DuplicateEmail);
        }

        let pass_hash = PasswordService::hash_password(&req.password)?;
        let user_id = Uuid::new_v4();

        let result = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, password_hash, display_name, role, created_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by
            "#
        )
        .bind(user_id)
        .bind(&email_norm)
        .bind(pass_hash)
        .bind(display_name)
        .bind(target_role.as_str())
        .bind(auth.id)
        .fetch_one(self.pool)
        .await;

        match result {
            Ok(user) => Ok(UserDto {
                id: user.id,
                email: user.email,
                display_name: user.display_name,
                role: user.role,
                is_active: user.is_active,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
                updated_at: user.updated_at,
            }),
            Err(sqlx::Error::Database(db_err)) => {
                let code = db_err.code().unwrap_or_default();
                let constraint = db_err.constraint().unwrap_or_default();
                let message = db_err.message();

                if code == "23505"
                    || constraint.contains("users_email")
                    || message.contains("users_email_key")
                    || message.contains("duplicate key")
                {
                    Err(AppError::DuplicateEmail)
                } else {
                    Err(AppError::DatabaseError(db_err.to_string()))
                }
            }
            Err(e) => Err(AppError::DatabaseError(e.to_string())),
        }
    }

    pub async fn toggle_active(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
        is_active: bool,
    ) -> AppResult<()> {
        if !auth.role.can_manage_users() {
            return Err(AppError::Forbidden);
        }

        // Prevent self-deactivation using authenticated user ID
        if auth.id == id && !is_active {
            return Err(AppError::ResourceConflict(
                "You cannot deactivate your own account".to_string(),
            ));
        }

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let target_user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::UserNotFound)?;

        let target_role = target_user.get_role();

        // Protect last active super admin from deactivation
        if target_role == Role::SuperAdmin && !is_active {
            let active_super_admins: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM users WHERE role = 'super_admin' AND is_active = TRUE",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            if active_super_admins.0 <= 1 {
                let _ = tx.rollback().await;
                return Err(AppError::ResourceConflict(
                    "Cannot deactivate the last active super_admin account".to_string(),
                ));
            }
        }

        sqlx::query(
            "UPDATE users SET is_active = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(is_active)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn update_user(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
        req: UpdateUserRequest,
    ) -> AppResult<UserDto> {
        if !auth.role.can_manage_users() {
            return Err(AppError::Forbidden);
        }

        if req.display_name.is_none() && req.role.is_none() && req.is_active.is_none() {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "root".to_string(),
                message:
                    "At least one field (display_name, role, is_active) must be provided for update"
                        .to_string(),
            }]));
        }

        let mut errors = Vec::new();

        let parsed_display_name = match req.display_name {
            Some(ref name) => {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    errors.push(ApiErrorDetails {
                        field: "display_name".to_string(),
                        message: "Display name cannot be empty".to_string(),
                    });
                    None
                } else if trimmed.len() < 2 || trimmed.len() > 100 {
                    errors.push(ApiErrorDetails {
                        field: "display_name".to_string(),
                        message: "Display name must be between 2 and 100 characters".to_string(),
                    });
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => None,
        };

        let parsed_role = match req.role {
            Some(ref r) => match Role::parse(r) {
                Some(role) => Some(role),
                None => {
                    errors.push(ApiErrorDetails {
                        field: "role".to_string(),
                        message: "Invalid role value. Accepted roles are 'admin' and 'super_admin'"
                            .to_string(),
                    });
                    None
                }
            },
            None => None,
        };

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        // Self-deactivation guard
        if auth.id == id && req.is_active == Some(false) {
            return Err(AppError::ResourceConflict(
                "You cannot deactivate your own account".to_string(),
            ));
        }

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let target_user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::UserNotFound)?;

        let current_role = target_user.get_role();
        let target_is_active = req.is_active.unwrap_or(target_user.is_active);
        let target_role_val = parsed_role.unwrap_or(current_role);

        // Protect last active super_admin from deactivation or demotion
        if current_role == Role::SuperAdmin
            && (!target_is_active || target_role_val != Role::SuperAdmin)
        {
            let active_super_admins: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM users WHERE role = 'super_admin' AND is_active = TRUE",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            if active_super_admins.0 <= 1 {
                let _ = tx.rollback().await;
                return Err(AppError::ResourceConflict(
                    "Cannot demote or deactivate the last active super_admin account".to_string(),
                ));
            }
        }

        let display_name_str = parsed_display_name.as_deref();
        let role_str = parsed_role.map(|r| r.as_str().to_string());

        let updated_user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET
                display_name = COALESCE($1, display_name),
                role = COALESCE($2, role),
                is_active = COALESCE($3, is_active),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $4
            RETURNING id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by
            "#
        )
        .bind(display_name_str)
        .bind(role_str)
        .bind(req.is_active)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(UserDto {
            id: updated_user.id,
            email: updated_user.email,
            display_name: updated_user.display_name,
            role: updated_user.role,
            is_active: updated_user.is_active,
            last_login_at: updated_user.last_login_at,
            created_at: updated_user.created_at,
            updated_at: updated_user.updated_at,
        })
    }
}
