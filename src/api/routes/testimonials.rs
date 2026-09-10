use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{
    AdminTestimonialDto, CreateTestimonialRequest, ReorderTestimonialsRequest,
    UpdateTestimonialRequest,
};
use crate::application::services::testimonial_service::TestimonialService;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

/// List testimonials
///
/// Returns all active testimonials in display sort order. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/testimonials",
    tag = "Testimonials",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of active testimonials", body = SingleResponse<Vec<AdminTestimonialDto>>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/testimonials")]
pub async fn list_testimonials(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<Vec<AdminTestimonialDto>>>> {
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    let list = service.list_testimonials(&auth).await?;
    Ok(Json(SingleResponse { data: list }))
}

/// Create testimonial
///
/// Creates a new testimonial. Sort order is automatically assigned to the next canonical sequence value. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/testimonials",
    tag = "Testimonials",
    request_body(content = CreateTestimonialRequest, description = "Testimonial creation payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Testimonial created successfully", body = SingleResponse<AdminTestimonialDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/testimonials", data = "<req>")]
pub async fn create_testimonial(
    auth: AuthenticatedUser,
    req: Json<CreateTestimonialRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<(Status, Json<SingleResponse<AdminTestimonialDto>>)> {
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    let dto = service.create_testimonial(&auth, req.into_inner()).await?;
    Ok((Status::Created, Json(SingleResponse { data: dto })))
}

/// Get testimonial
///
/// Returns details of a single testimonial by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/testimonials/{id}",
    tag = "Testimonials",
    params(
        ("id" = Uuid, Path, description = "Testimonial UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Testimonial details", body = SingleResponse<AdminTestimonialDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Testimonial not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/testimonials/<id_str>")]
pub async fn get_testimonial(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminTestimonialDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid testimonial ID".to_string(),
        }])
    })?;
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    let dto = service.get_testimonial(&auth, id).await?;
    Ok(Json(SingleResponse { data: dto }))
}

/// Update testimonial
///
/// Updates text, author, avatar, or visibility of a testimonial. Explicit null in avatar_media_id clears the avatar. Requires authenticated super_admin or admin.
#[utoipa::path(
    patch,
    path = "/api/v1/admin/testimonials/{id}",
    tag = "Testimonials",
    params(
        ("id" = Uuid, Path, description = "Testimonial UUID identifier")
    ),
    request_body(content = UpdateTestimonialRequest, description = "Testimonial update payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Testimonial updated successfully", body = SingleResponse<AdminTestimonialDto>),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Testimonial not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::patch("/admin/testimonials/<id_str>", data = "<req>")]
pub async fn update_testimonial(
    auth: AuthenticatedUser,
    id_str: &str,
    req: Json<UpdateTestimonialRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminTestimonialDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid testimonial ID".to_string(),
        }])
    })?;
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    let updated = service
        .update_testimonial(&auth, id, req.into_inner())
        .await?;
    Ok(Json(SingleResponse { data: updated }))
}

/// Delete testimonial
///
/// Soft deletes a testimonial by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/testimonials/{id}",
    tag = "Testimonials",
    params(
        ("id" = Uuid, Path, description = "Testimonial UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Testimonial deleted successfully"),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Testimonial not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::delete("/admin/testimonials/<id_str>")]
pub async fn delete_testimonial(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Status> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid testimonial ID".to_string(),
        }])
    })?;
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    service.delete_testimonial(&auth, id).await?;
    Ok(Status::Ok)
}

/// Reorder testimonials
///
/// Updates display sort order of testimonials. Provided sort orders serve as ordering hints and are normalized into a canonical 10-step sequence. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/testimonials/reorder",
    tag = "Testimonials",
    request_body(content = ReorderTestimonialsRequest, description = "Reorder items array containing testimonial IDs and sort orders"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Testimonials reordered successfully"),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/testimonials/reorder", data = "<req>")]
pub async fn reorder_testimonials(
    auth: AuthenticatedUser,
    req: Json<ReorderTestimonialsRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Status> {
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    service
        .reorder_testimonials(&auth, req.into_inner())
        .await?;
    Ok(Status::Ok)
}
