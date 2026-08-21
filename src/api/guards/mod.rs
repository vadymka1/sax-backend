use rocket::request::{FromRequest, Outcome, Request};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::domain::users::{Role, User};
use crate::infrastructure::auth::TokenService;
use crate::shared::errors::AppError;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub is_active: bool,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = AppError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = match req.rocket().state::<AppConfig>() {
            Some(c) => c,
            None => {
                return Outcome::Error((
                    rocket::http::Status::InternalServerError,
                    AppError::Internal("Config state missing".to_string()),
                ))
            }
        };

        let db = match req.rocket().state::<PgPool>() {
            Some(p) => p,
            None => {
                return Outcome::Error((
                    rocket::http::Status::InternalServerError,
                    AppError::Internal("Database pool state missing".to_string()),
                ))
            }
        };

        let auth_header = match req.headers().get_one("Authorization") {
            Some(h) => h,
            None => {
                return Outcome::Error((rocket::http::Status::Unauthorized, AppError::Unauthorized))
            }
        };

        if !auth_header.starts_with("Bearer ") {
            return Outcome::Error((rocket::http::Status::Unauthorized, AppError::Unauthorized));
        }

        let token = &auth_header[7..];
        let claims = match TokenService::decode_access_token(token, &config.jwt_access_secret) {
            Ok(c) => c,
            Err(e) => return Outcome::Error((e.status(), e)),
        };

        let user_id = match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => {
                return Outcome::Error((rocket::http::Status::Unauthorized, AppError::Unauthorized))
            }
        };

        // Always query database to verify user status and get current actual role
        let user = match sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(db)
        .await
        {
            Ok(Some(u)) => u,
            Ok(None) => return Outcome::Error((rocket::http::Status::Unauthorized, AppError::Unauthorized)),
            Err(e) => return Outcome::Error((rocket::http::Status::InternalServerError, AppError::DatabaseError(e.to_string()))),
        };

        if !user.is_active {
            return Outcome::Error((rocket::http::Status::Forbidden, AppError::UserInactive));
        }

        let role = user.get_role();

        Outcome::Success(AuthenticatedUser {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            role,
            is_active: user.is_active,
        })
    }
}

pub struct RequireRole(pub Role);

pub fn ensure_role(user: &AuthenticatedUser, required: Role) -> Result<(), AppError> {
    match required {
        Role::SuperAdmin => {
            if user.role == Role::SuperAdmin {
                Ok(())
            } else {
                Err(AppError::Forbidden)
            }
        }
        Role::Admin => {
            if matches!(user.role, Role::SuperAdmin | Role::Admin) {
                Ok(())
            } else {
                Err(AppError::Forbidden)
            }
        }
        Role::Editor => Ok(()),
    }
}
