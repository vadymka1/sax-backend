use sqlx::{PgPool, Postgres, Transaction};
use validator::ValidateEmail;

use crate::application::dto::{
    AuthTokensDto, LoginRequest, LogoutRequest, RefreshTokenRequest, UserDto,
};
use crate::config::AppConfig;
use crate::domain::users::User;
use crate::infrastructure::auth::{PasswordService, TokenService};
use crate::shared::errors::{AppError, AppResult};

#[derive(sqlx::FromRow)]
struct RefreshTokenRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct AuthService<'a> {
    pool: &'a PgPool,
    config: &'a AppConfig,
}

impl<'a> AuthService<'a> {
    pub fn new(pool: &'a PgPool, config: &'a AppConfig) -> Self {
        Self { pool, config }
    }

    pub async fn login(&self, req: LoginRequest) -> AppResult<AuthTokensDto> {
        let email_norm = req.email.trim().to_lowercase();
        if !ValidateEmail::validate_email(&email_norm) {
            return Err(AppError::InvalidCredentials);
        }

        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE email = $1"
        )
        .bind(&email_norm)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::InvalidCredentials)?;

        if !user.is_active {
            return Err(AppError::UserInactive);
        }

        if !PasswordService::verify_password(&req.password, &user.password_hash) {
            return Err(AppError::InvalidCredentials);
        }

        let role = user.get_role();
        let access_token = TokenService::generate_access_token(
            user.id,
            &user.email,
            role,
            &self.config.jwt_access_secret,
            self.config.jwt_access_ttl_seconds,
        )?;

        let refresh_token = TokenService::generate_refresh_token();
        let refresh_hash = TokenService::hash_refresh_token(&refresh_token);
        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(self.config.jwt_refresh_ttl_seconds);

        sqlx::query(
            "INSERT INTO user_refresh_tokens (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(uuid::Uuid::new_v4())
        .bind(user.id)
        .bind(&refresh_hash)
        .bind(expires_at)
        .execute(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if let Err(err) =
            sqlx::query("UPDATE users SET last_login_at = CURRENT_TIMESTAMP WHERE id = $1")
                .bind(user.id)
                .execute(self.pool)
                .await
        {
            tracing::warn!(
                user_id = %user.id,
                error = %err,
                "failed to update user last_login_at"
            );
        }

        let user_dto = UserDto {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            is_active: user.is_active,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        };

        Ok(AuthTokensDto {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.jwt_access_ttl_seconds,
            user: user_dto,
        })
    }

    pub async fn refresh(&self, req: RefreshTokenRequest) -> AppResult<(String, String)> {
        let old_hash = TokenService::hash_refresh_token(&req.refresh_token);

        let mut tx: Transaction<'_, Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let record = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT id, user_id, expires_at, revoked_at FROM user_refresh_tokens WHERE token_hash = $1 FOR UPDATE"
        )
        .bind(&old_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::TokenRevoked)?;

        if record.revoked_at.is_some() || record.expires_at < chrono::Utc::now() {
            let _ = tx.rollback().await;
            return Err(AppError::TokenRevoked);
        }

        sqlx::query("UPDATE user_refresh_tokens SET revoked_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(record.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
        )
        .bind(record.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::UserInactive)?;

        if !user.is_active {
            let _ = tx.rollback().await;
            return Err(AppError::UserInactive);
        }

        let new_access = TokenService::generate_access_token(
            user.id,
            &user.email,
            user.get_role(),
            &self.config.jwt_access_secret,
            self.config.jwt_access_ttl_seconds,
        )?;

        let new_refresh = TokenService::generate_refresh_token();
        let new_hash = TokenService::hash_refresh_token(&new_refresh);
        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(self.config.jwt_refresh_ttl_seconds);

        sqlx::query(
            "INSERT INTO user_refresh_tokens (id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(uuid::Uuid::new_v4())
        .bind(user.id)
        .bind(&new_hash)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok((new_access, new_refresh))
    }

    pub async fn logout(&self, req: LogoutRequest) -> AppResult<()> {
        let hash = TokenService::hash_refresh_token(&req.refresh_token);
        sqlx::query("UPDATE user_refresh_tokens SET revoked_at = CURRENT_TIMESTAMP WHERE token_hash = $1 AND revoked_at IS NULL")
            .bind(&hash)
            .execute(self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
