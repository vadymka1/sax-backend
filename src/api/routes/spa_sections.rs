use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminSpaSectionDto, CreateSpaSectionRequest, ReorderContentBlocksRequest,
    ReorderSpaSectionsRequest, UpdateSpaSectionRequest,
};
use crate::application::services::content_block_service::ContentBlockService;
use crate::application::services::spa_section_service::SpaSectionService;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

/// List SPA sections
///
/// Returns all SPA sections configured for the home page in display order. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/spa-sections",
    tag = "SPA Sections",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of SPA sections", body = SingleResponse<Vec<AdminSpaSectionDto>>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/spa-sections")]
pub async fn list_spa_sections(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<Vec<AdminSpaSectionDto>>>> {
    let service = SpaSectionService::new(db.inner());
    let sections = service.list_admin_sections(&auth).await?;
    Ok(Json(SingleResponse { data: sections }))
}

/// Create SPA section
///
/// Creates a new dynamic SPA section for the home page. Automatically generates a unique slug section key. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections",
    tag = "SPA Sections",
    request_body(content = CreateSpaSectionRequest, description = "SPA section creation payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "SPA section created successfully", body = SingleResponse<AdminSpaSectionDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/spa-sections", data = "<req>")]
pub async fn create_spa_section(
    auth: AuthenticatedUser,
    req: Json<CreateSpaSectionRequest>,
    db: &State<PgPool>,
) -> AppResult<(Status, Json<SingleResponse<AdminSpaSectionDto>>)> {
    let service = SpaSectionService::new(db.inner());
    let dto = service.create_section(&auth, req.into_inner()).await?;
    Ok((Status::Created, Json(SingleResponse { data: dto })))
}

/// Get SPA section
///
/// Returns details of a single SPA section by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/spa-sections/{id}",
    tag = "SPA Sections",
    params(
        ("id" = Uuid, Path, description = "SPA section UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "SPA section details", body = SingleResponse<AdminSpaSectionDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "SPA section not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/spa-sections/<id_str>")]
pub async fn get_spa_section(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<AdminSpaSectionDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid section ID".to_string(),
        }])
    })?;
    let service = SpaSectionService::new(db.inner());
    let section = service.get_admin_section(&auth, id).await?;
    Ok(Json(SingleResponse { data: section }))
}

/// Update SPA section
///
/// Updates title, navigation label, or visibility status of a SPA section. Note that section key remains immutable after creation. Requires authenticated super_admin or admin.
#[utoipa::path(
    patch,
    path = "/api/v1/admin/spa-sections/{id}",
    tag = "SPA Sections",
    params(
        ("id" = Uuid, Path, description = "SPA section UUID identifier")
    ),
    request_body(content = UpdateSpaSectionRequest, description = "SPA section update payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "SPA section updated successfully", body = SingleResponse<AdminSpaSectionDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "SPA section not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::patch("/admin/spa-sections/<id_str>", data = "<req>")]
pub async fn update_spa_section(
    auth: AuthenticatedUser,
    id_str: &str,
    req: Json<UpdateSpaSectionRequest>,
    db: &State<PgPool>,
) -> AppResult<Json<SingleResponse<AdminSpaSectionDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid section ID".to_string(),
        }])
    })?;
    let service = SpaSectionService::new(db.inner());
    let updated = service.update_section(&auth, id, req.into_inner()).await?;
    Ok(Json(SingleResponse { data: updated }))
}

/// Delete SPA section
///
/// Soft deletes an empty SPA section by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/spa-sections/{id}",
    tag = "SPA Sections",
    params(
        ("id" = Uuid, Path, description = "SPA section UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "SPA section deleted successfully"),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "SPA section not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::delete("/admin/spa-sections/<id_str>")]
pub async fn delete_spa_section(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
) -> AppResult<Status> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid section ID".to_string(),
        }])
    })?;
    let service = SpaSectionService::new(db.inner());
    service.delete_section(&auth, id).await?;
    Ok(Status::Ok)
}

/// Reorder SPA sections
///
/// Updates display sort order of SPA sections. Provided sort orders are authoritative. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections/reorder",
    tag = "SPA Sections",
    request_body(content = ReorderSpaSectionsRequest, description = "Reorder items array containing section IDs and sort orders"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "SPA sections reordered successfully"),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/spa-sections/reorder", data = "<req>")]
pub async fn reorder_spa_sections(
    auth: AuthenticatedUser,
    req: Json<ReorderSpaSectionsRequest>,
    db: &State<PgPool>,
) -> AppResult<Status> {
    let service = SpaSectionService::new(db.inner());
    service.reorder_sections(&auth, req.into_inner()).await?;
    Ok(Status::Ok)
}

/// Reorder section content blocks
///
/// Per-section transactional reordering of content blocks within a specific target SPA section. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections/{spa_section_id}/content-blocks/reorder",
    tag = "SPA Sections",
    params(
        ("spa_section_id" = Uuid, Path, description = "Target SPA section UUID identifier")
    ),
    request_body(content = ReorderContentBlocksRequest, description = "Reorder items array containing block IDs and sort orders"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Section content blocks reordered successfully"),
        (status = 422, description = "Validation error or invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post(
    "/admin/spa-sections/<section_id_str>/content-blocks/reorder",
    data = "<req>"
)]
pub async fn reorder_section_content_blocks(
    auth: AuthenticatedUser,
    section_id_str: &str,
    req: Json<ReorderContentBlocksRequest>,
    db: &State<PgPool>,
) -> AppResult<Status> {
    let section_id = Uuid::parse_str(section_id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "spa_section_id".to_string(),
            message: "Invalid SPA section ID".to_string(),
        }])
    })?;
    let service = ContentBlockService::new(db.inner());
    service
        .reorder_section_blocks(&auth, section_id, req.into_inner())
        .await?;
    Ok(Status::Ok)
}
