use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{CreateUserRequest, UserDto};
use crate::application::services::admin_user_service::AdminUserService;
use crate::config::AppConfig;
use crate::shared::errors::{AppError, AppResult};
use crate::shared::pagination::{PaginatedResponse, SingleResponse};

#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    params(
        ("page" = Option<i64>, Query, description = "Page number"),
        ("page_size" = Option<i64>, Query, description = "Items per page")
    ),
    responses((status = 200, body = PaginatedResponse<UserDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/users",
    request_body = CreateUserRequest,
    responses((status = 200, body = SingleResponse<UserDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/activate",
    responses((status = 200)),
    security(("bearer_auth" = []))
)]
#[rocket::post("/admin/users/<id_str>/activate")]
pub async fn activate_user(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<serde_json::Value>> {
    let id = Uuid::parse_str(id_str).map_err(|_| AppError::ValidationError(vec![]))?;
    let service = AdminUserService::new(db.inner(), config.inner());
    service.toggle_active(&auth, id, true).await?;
    Ok(Json(
        serde_json::json!({ "data": { "message": "User activated successfully" } }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/deactivate",
    responses((status = 200)),
    security(("bearer_auth" = []))
)]
#[rocket::post("/admin/users/<id_str>/deactivate")]
pub async fn deactivate_user(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
) -> AppResult<Json<serde_json::Value>> {
    let id = Uuid::parse_str(id_str).map_err(|_| AppError::ValidationError(vec![]))?;
    let service = AdminUserService::new(db.inner(), config.inner());
    service.toggle_active(&auth, id, false).await?;
    Ok(Json(
        serde_json::json!({ "data": { "message": "User deactivated successfully" } }),
    ))
}
