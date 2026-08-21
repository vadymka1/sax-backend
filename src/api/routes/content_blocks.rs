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

#[utoipa::path(
    get,
    path = "/api/v1/admin/content-blocks",
    params(
        ("spa_section_id" = Option<Uuid>, Query, description = "Filter content blocks by target SPA section ID")
    ),
    responses((status = 200, body = SingleResponse<Vec<AdminContentBlockDto>>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/content-blocks/{id}",
    responses((status = 200, body = SingleResponse<AdminContentBlockDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/content-blocks",
    request_body = CreateContentBlockRequest,
    responses((status = 201, body = SingleResponse<AdminContentBlockDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    patch,
    path = "/api/v1/admin/content-blocks/{id}",
    request_body = UpdateContentBlockRequest,
    responses((status = 200, body = SingleResponse<AdminContentBlockDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    delete,
    path = "/api/v1/admin/content-blocks/{id}",
    responses((status = 200)),
    security(("bearer_auth" = []))
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
