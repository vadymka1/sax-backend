use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{CreateUserRequest, MessageDataDto, UpdateUserRequest, UserDto};
use crate::application::services::admin_user_service::AdminUserService;
use crate::config::AppConfig;
use crate::shared::errors::{AppError, AppResult};
use crate::shared::pagination::{PaginatedResponse, SingleResponse};

/// List admin users
///
/// Returns a paginated list of registered admin users. Requires authenticated super_admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    tag = "Admin Users",
    params(
        ("page" = Option<i64>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<i64>, Query, description = "Items per page (default 20, min 1, max 100)")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Paginated list of admin users", body = PaginatedResponse<UserDto>),
        (status = 422, description = "Invalid pagination query parameters", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/users?<page>&<page_size>")]
pub async fn list_users(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> AppResult<Json<PaginatedResponse<UserDto>>> {
    let service = AdminUserService::new(db.inner(), config.inner());
    let response = service.list_users(&auth, page, page_size).await?;
    Ok(Json(response))
}

/// Create admin user
///
/// Creates a new admin user account with Argon2id hashed password. Requires authenticated super_admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/users",
    tag = "Admin Users",
    request_body(content = CreateUserRequest, description = "New user creation payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Admin user created successfully", body = SingleResponse<UserDto>),
        (status = 422, description = "Validation failure", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin role", body = ApiErrorResponse),
        (status = 409, description = "Duplicate email address", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/users", data = "<req>")]
pub async fn create_user(
    auth: AuthenticatedUser,
    req: Json<CreateUserRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<UserDto>>> {
    let service = AdminUserService::new(db.inner(), config.inner());
    let dto = service.create_admin(&auth, req.into_inner()).await?;
    Ok(Json(SingleResponse { data: dto }))
}

/// Update admin user
///
/// Updates display_name, role, or active status of an admin user account by UUID. Requires authenticated super_admin.
#[utoipa::path(
    patch,
    path = "/api/v1/admin/users/{id}",
    tag = "Admin Users",
    params(
        ("id" = Uuid, Path, description = "User UUID identifier")
    ),
    request_body(content = UpdateUserRequest, description = "User update payload with optional display_name, role, and is_active fields"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Admin user updated successfully", body = SingleResponse<UserDto>),
        (status = 422, description = "Validation error or invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin role", body = ApiErrorResponse),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 409, description = "Conflict - Cannot deactivate or demote last active super_admin or deactivate self", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::patch("/admin/users/<id_str>", data = "<req>")]
pub async fn update_user(
    auth: AuthenticatedUser,
    id_str: &str,
    req: Json<UpdateUserRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<UserDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| AppError::ValidationError(vec![]))?;
    let service = AdminUserService::new(db.inner(), config.inner());
    let dto = service.update_user(&auth, id, req.into_inner()).await?;
    Ok(Json(SingleResponse { data: dto }))
}

/// Activate admin user
///
/// Activates an admin user account by UUID. Requires authenticated super_admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/activate",
    tag = "Admin Users",
    params(
        ("id" = Uuid, Path, description = "User UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "User activated successfully", body = SingleResponse<MessageDataDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin role", body = ApiErrorResponse),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/users/<id_str>/activate")]
pub async fn activate_user(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<MessageDataDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| AppError::ValidationError(vec![]))?;
    let service = AdminUserService::new(db.inner(), config.inner());
    service.toggle_active(&auth, id, true).await?;
    Ok(Json(SingleResponse {
        data: MessageDataDto {
            message: "User activated successfully".to_string(),
        },
    }))
}

/// Deactivate admin user
///
/// Deactivates an admin user account by UUID. Self-deactivation and deactivating the last active super_admin are protected and return 409 Conflict. Requires authenticated super_admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/deactivate",
    tag = "Admin Users",
    params(
        ("id" = Uuid, Path, description = "User UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "User deactivated successfully", body = SingleResponse<MessageDataDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin role", body = ApiErrorResponse),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 409, description = "Conflict - Cannot deactivate self or last active super_admin", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/users/<id_str>/deactivate")]
pub async fn deactivate_user(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<SingleResponse<MessageDataDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| AppError::ValidationError(vec![]))?;
    let service = AdminUserService::new(db.inner(), config.inner());
    service.toggle_active(&auth, id, false).await?;
    Ok(Json(SingleResponse {
        data: MessageDataDto {
            message: "User deactivated successfully".to_string(),
        },
    }))
}
