use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminContentBlockDto, CreateContentBlockRequest, UpdateContentBlockRequest,
};
use crate::application::services::content_block_service::ContentBlockService;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

/// List content blocks
///
/// Returns home page content blocks, optionally filtered by SPA section UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/content-blocks",
    tag = "Content Blocks",
    params(
        ("spa_section_id" = Option<Uuid>, Query, description = "Filter content blocks by target SPA section UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of content blocks", body = SingleResponse<Vec<AdminContentBlockDto>>),
        (status = 422, description = "Invalid UUID query parameter format", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/content-blocks?<spa_section_id>")]
pub async fn list_content_blocks(
    auth: AuthenticatedUser,
    spa_section_id: Option<&str>,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<Vec<AdminContentBlockDto>>>> {
    let filter_id = match spa_section_id {
        Some(s) => Some(Uuid::parse_str(s).map_err(|_| {
            AppError::ValidationError(vec![ApiErrorDetails {
                field: "spa_section_id".to_string(),
                message: "Invalid spa_section_id query parameter format".to_string(),
            }])
        })?),
        None => None,
    };
    let service = ContentBlockService::new(db.inner());
    let blocks = service.list_blocks(&auth, filter_id).await?;
    Ok(Json(SingleResponse { data: blocks }))
}

/// Get content block
///
/// Returns details of a single content block by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/content-blocks/{id}",
    tag = "Content Blocks",
    params(
        ("id" = Uuid, Path, description = "Content block UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Content block details", body = SingleResponse<AdminContentBlockDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Content block not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/content-blocks/<id_str>")]
pub async fn get_content_block(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<AdminContentBlockDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid block ID".to_string(),
        }])
    })?;
    let service = ContentBlockService::new(db.inner());
    let block = service.get_block(&auth, id).await?;
    Ok(Json(SingleResponse { data: block }))
}

/// Create content block
///
/// Creates a new content block attached to a target SPA section. Optionally associates a media asset ID. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/content-blocks",
    tag = "Content Blocks",
    request_body(content = CreateContentBlockRequest, description = "Content block creation payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Content block created successfully", body = SingleResponse<AdminContentBlockDto>),
        (status = 422, description = "Validation error or invalid SPA section / media reference", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/content-blocks", data = "<req>")]
pub async fn create_content_block(
    auth: AuthenticatedUser,
    req: Json<CreateContentBlockRequest>,
    db: &State<PgPool>,
) -> AppResult<(Status, Json<SingleResponse<AdminContentBlockDto>>)> {
    let service = ContentBlockService::new(db.inner());
    let block = service.create_block(&auth, req.into_inner()).await?;
    Ok((Status::Created, Json(SingleResponse { data: block })))
}

/// Update content block
///
/// Updates fields, section assignment, or attached media asset of a content block. Requires authenticated super_admin or admin.
#[utoipa::path(
    patch,
    path = "/api/v1/admin/content-blocks/{id}",
    tag = "Content Blocks",
    params(
        ("id" = Uuid, Path, description = "Content block UUID identifier")
    ),
    request_body(content = UpdateContentBlockRequest, description = "Content block update payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Content block updated successfully", body = SingleResponse<AdminContentBlockDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Content block not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::patch("/admin/content-blocks/<id_str>", data = "<req>")]
pub async fn update_content_block(
    auth: AuthenticatedUser,
    id_str: &str,
    req: Json<UpdateContentBlockRequest>,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<AdminContentBlockDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid block ID".to_string(),
        }])
    })?;
    let service = ContentBlockService::new(db.inner());
    let block = service.update_block(&auth, id, req.into_inner()).await?;
    Ok(Json(SingleResponse { data: block }))
}

/// Delete content block
///
/// Soft deletes a content block by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/content-blocks/{id}",
    tag = "Content Blocks",
    params(
        ("id" = Uuid, Path, description = "Content block UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Content block deleted successfully"),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Content block not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::delete("/admin/content-blocks/<id_str>")]
pub async fn delete_content_block(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
) -> AppResult<Status> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid block ID".to_string(),
        }])
    })?;
    let service = ContentBlockService::new(db.inner());
    service.delete_block(&auth, id).await?;
    Ok(Status::Ok)
}
