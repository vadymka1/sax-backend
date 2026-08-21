use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AuthTokensDto, LoginRequest, LogoutRequest, RefreshTokenRequest, UserDto,
};
use crate::application::services::auth_service::AuthService;
use crate::config::AppConfig;
use crate::shared::errors::AppResult;
use crate::shared::pagination::SingleResponse;

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, body = SingleResponse<AuthTokensDto>))
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    request_body = RefreshTokenRequest,
    responses((status = 200))
)]
#[rocket::post("/auth/refresh", data = "<refresh_req>")]
pub async fn refresh(
    refresh_req: Json<RefreshTokenRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<serde_json::Value>> {
    let service = AuthService::new(db.inner(), config.inner());
    let (access_token, refresh_token) = service.refresh(refresh_req.into_inner()).await?;

    Ok(Json(serde_json::json!({
        "data": {
            "access_token": access_token,
            "refresh_token": refresh_token,
            "token_type": "Bearer",
            "expires_in": config.jwt_access_ttl_seconds
        }
    })))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    request_body = LogoutRequest,
    responses((status = 200))
)]
#[rocket::post("/auth/logout", data = "<logout_req>")]
pub async fn logout(
    logout_req: Json<LogoutRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<serde_json::Value>> {
    let service = AuthService::new(db.inner(), config.inner());
    service.logout(logout_req.into_inner()).await?;
    Ok(Json(
        serde_json::json!({ "data": { "message": "Successfully logged out" } }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses((status = 200, body = SingleResponse<UserDto>))
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
