use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AuthTokensDto, LoginRequest, LogoutRequest, MessageDataDto, RefreshTokenDataDto,
    RefreshTokenRequest, UserDto,
};
use crate::application::services::auth_service::AuthService;
use crate::config::AppConfig;
use crate::shared::errors::AppResult;
use crate::shared::pagination::SingleResponse;

/// User login
///
/// Authenticates super admin or admin user using email and password credentials. Returns short-lived access JWT and refresh token in JSON response envelope.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Auth",
    request_body(content = LoginRequest, description = "Login credentials (email and password)"),
    responses(
        (status = 200, description = "Authentication successful", body = SingleResponse<AuthTokensDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Invalid credentials or inactive account", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/login", data = "<login_req>")]
pub async fn login(
    login_req: Json<LoginRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<AuthTokensDto>>> {
    let service = AuthService::new(db.inner(), config.inner());
    let tokens = service.login(login_req.into_inner()).await?;
    Ok(Json(SingleResponse { data: tokens }))
}

/// Refresh access token
///
/// Exchanges a valid refresh token for a new access token and rotated refresh token pair.
#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "Auth",
    request_body(content = RefreshTokenRequest, description = "Active refresh token"),
    responses(
        (status = 200, description = "Token refreshed successfully", body = SingleResponse<RefreshTokenDataDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Invalid, expired, or revoked refresh token", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/refresh", data = "<refresh_req>")]
pub async fn refresh(
    refresh_req: Json<RefreshTokenRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<RefreshTokenDataDto>>> {
    let service = AuthService::new(db.inner(), config.inner());
    let (access_token, refresh_token) = service.refresh(refresh_req.into_inner()).await?;

    Ok(Json(SingleResponse {
        data: RefreshTokenDataDto {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: config.jwt_access_ttl_seconds,
        },
    }))
}

/// User logout
///
/// Revokes the provided refresh token and invalidates active session.
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "Auth",
    request_body(content = LogoutRequest, description = "Refresh token to revoke"),
    responses(
        (status = 200, description = "Successfully logged out", body = SingleResponse<MessageDataDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Invalid or revoked refresh token", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/logout", data = "<logout_req>")]
pub async fn logout(
    logout_req: Json<LogoutRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<MessageDataDto>>> {
    let service = AuthService::new(db.inner(), config.inner());
    service.logout(logout_req.into_inner()).await?;
    Ok(Json(SingleResponse {
        data: MessageDataDto {
            message: "Successfully logged out".to_string(),
        },
    }))
}

/// Get current user profile
///
/// Returns profile and role information for the currently authenticated user.
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "Auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated user profile", body = SingleResponse<UserDto>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/auth/me")]
pub async fn get_me(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<UserDto>>> {
    let user = sqlx::query_as::<_, crate::domain::users::User>(
        "SELECT id, email, password_hash, display_name, role, is_active, last_login_at, created_at, updated_at, created_by FROM users WHERE id = $1"
    )
    .bind(auth.id)
    .fetch_one(db.inner())
    .await
    .map_err(|e| crate::shared::errors::AppError::DatabaseError(e.to_string()))?;

    let dto = UserDto {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        role: user.role,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
    };

    Ok(Json(SingleResponse { data: dto }))
}
