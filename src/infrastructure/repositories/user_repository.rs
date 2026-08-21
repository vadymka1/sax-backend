use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::domain::users::{Role, User};
use crate::shared::errors::{AppError, AppResult};

pub struct UserRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> UserRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        let email_lower = email.trim().to_lowercase();
        sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE LOWER(email) = $1"
        )
        .bind(&email_lower)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn count(&self) -> AppResult<i64> {
        let res: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        Ok(res.0)
    }

    pub async fn list(&self, limit: i64, offset: i64) -> AppResult<Vec<User>> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn create(
        &self,
        email: &str,
        password_hash: &str,
        display_name: &str,
        role: Role,
        created_by: Uuid,
    ) -> AppResult<User> {
        let email_lower = email.trim().to_lowercase();
        let user_id = Uuid::new_v4();

        info!("Creating user {} with role {}", email_lower, role.as_str());

        sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, password_hash, display_name, role, created_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by
            "#
        )
        .bind(user_id)
        .bind(&email_lower)
        .bind(password_hash)
        .bind(display_name)
        .bind(role.as_str())
        .bind(created_by)
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                AppError::DuplicateEmail
            } else {
                AppError::DatabaseError(e.to_string())
            }
        })
    }

    pub async fn update_status(&self, id: Uuid, is_active: bool) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET is_active = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(is_active)
        .bind(id)
        .execute(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
