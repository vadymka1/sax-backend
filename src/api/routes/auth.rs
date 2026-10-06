use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::serde::json::Json;
use rocket::time::Duration;
use rocket::State;
use sqlx::PgPool;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AuthTokensDto, LoginRequest, MessageDataDto, RefreshTokenDataDto, UserDto,
};
use crate::application::services::auth_service::AuthService;
use crate::config::AppConfig;
use crate::shared::errors::{AppError, AppResult};
use crate::shared::pagination::SingleResponse;

pub const REFRESH_COOKIE_NAME: &str = "refresh_token";
pub const REFRESH_COOKIE_PATH: &str = "/api/v1/auth";

pub fn build_refresh_cookie(token: &str, ttl_seconds: i64, is_secure: bool) -> Cookie<'static> {
    Cookie::build((REFRESH_COOKIE_NAME, token.to_string()))
        .path(REFRESH_COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(is_secure)
        .max_age(Duration::seconds(ttl_seconds))
        .build()
}

pub fn build_clear_refresh_cookie<'a>(is_secure: bool) -> Cookie<'a> {
    Cookie::build((REFRESH_COOKIE_NAME, ""))
        .path(REFRESH_COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(is_secure)
        .max_age(Duration::seconds(0))
        .build()
}

pub fn set_refresh_cookie(jar: &CookieJar<'_>, token: &str, ttl_seconds: i64, is_secure: bool) {
    jar.add(build_refresh_cookie(token, ttl_seconds, is_secure));
}

pub fn clear_refresh_cookie(jar: &CookieJar<'_>, is_secure: bool) {
    jar.remove(build_clear_refresh_cookie(is_secure));
}

/// User login
///
/// Authenticates super admin or admin user using email and password credentials.
/// Returns short-lived access JWT in JSON response envelope and sets HttpOnly refresh token cookie.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Auth",
    request_body(content = LoginRequest, description = "Login credentials (email and password)"),
    responses(
        (status = 200, description = "Authentication successful (access token in JSON, refresh token set in HttpOnly cookie)", body = SingleResponse<AuthTokensDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Invalid credentials or inactive account", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/login", data = "<login_req>")]
pub async fn login(
    login_req: Json<LoginRequest>,
    jar: &CookieJar<'_>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<AuthTokensDto>>> {
    let service = AuthService::new(db.inner(), config.inner());
    let (tokens, refresh_token) = service.login(login_req.into_inner()).await?;
    set_refresh_cookie(
        jar,
        &refresh_token,
        config.jwt_refresh_ttl_seconds,
        config.is_production(),
    );
    Ok(Json(SingleResponse { data: tokens }))
}

/// Refresh access token
///
/// Exchanges an active HttpOnly refresh cookie for a new access token and rotated refresh cookie.
/// Does not accept or require a JSON request body.
#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "Auth",
    responses(
        (status = 200, description = "Token refreshed successfully (new access token in JSON, rotated refresh token in HttpOnly cookie)", body = SingleResponse<RefreshTokenDataDto>),
        (status = 401, description = "Missing, invalid, expired, or revoked refresh cookie", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/refresh")]
pub async fn refresh(
    jar: &CookieJar<'_>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<RefreshTokenDataDto>>> {
    let cookie = jar.get(REFRESH_COOKIE_NAME).ok_or(AppError::Unauthorized)?;

    let token = cookie.value().trim();
    if token.is_empty() {
        return Err(AppError::Unauthorized);
    }

    let service = AuthService::new(db.inner(), config.inner());
    let (dto, new_refresh) = service.refresh(token).await?;

    set_refresh_cookie(
        jar,
        &new_refresh,
        config.jwt_refresh_ttl_seconds,
        config.is_production(),
    );

    Ok(Json(SingleResponse { data: dto }))
}

/// User logout
///
/// Revokes active session associated with HttpOnly refresh cookie and clears the cookie.
/// Does not require a JSON request body; handles missing cookie safely and idempotently.
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "Auth",
    responses(
        (status = 200, description = "Successfully logged out and session cleared", body = SingleResponse<MessageDataDto>),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/auth/logout")]
pub async fn logout(
    jar: &CookieJar<'_>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<MessageDataDto>>> {
    if let Some(cookie) = jar.get(REFRESH_COOKIE_NAME) {
        let token = cookie.value().trim();
        if !token.is_empty() {
            let service = AuthService::new(db.inner(), config.inner());
            let _ = service.logout(token).await;
        }
    }

    clear_refresh_cookie(jar, config.is_production());

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
