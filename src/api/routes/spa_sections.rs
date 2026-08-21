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

#[utoipa::path(
    get,
    path = "/api/v1/admin/spa-sections",
    responses((status = 200, body = SingleResponse<Vec<AdminSpaSectionDto>>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections",
    request_body = CreateSpaSectionRequest,
    responses((status = 201, body = SingleResponse<AdminSpaSectionDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/spa-sections/{id}",
    responses((status = 200, body = SingleResponse<AdminSpaSectionDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    patch,
    path = "/api/v1/admin/spa-sections/{id}",
    request_body = UpdateSpaSectionRequest,
    responses((status = 200, body = SingleResponse<AdminSpaSectionDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    delete,
    path = "/api/v1/admin/spa-sections/{id}",
    responses((status = 200)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections/reorder",
    request_body = ReorderSpaSectionsRequest,
    responses((status = 200)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/spa-sections/{spa_section_id}/content-blocks/reorder",
    request_body = ReorderContentBlocksRequest,
    responses((status = 200)),
    security(("bearer_auth" = []))
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
