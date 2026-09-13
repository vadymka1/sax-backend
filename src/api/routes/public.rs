use std::sync::Arc;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;

use crate::application::dto::{
    PublicPageResponse, SubmitPublicTestimonialRequest, SubmitPublicTestimonialResponse,
};
use crate::application::services::testimonial_service::TestimonialService;
use crate::application::services::PublicPageService;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::AppResult;
use crate::shared::pagination::SingleResponse;

/// Get aggregated public SPA page
///
/// Returns home page details, visible SPA sections, ordered content blocks, and their associated media in a single public response envelope.
#[utoipa::path(
    get,
    path = "/api/v1/public/page",
    tag = "Public",
    responses(
        (status = 200, description = "Aggregated public SPA home page payload", body = SingleResponse<PublicPageResponse>),
        (status = 500, description = "Internal server error", body = crate::shared::errors::ApiErrorResponse)
    )
)]
#[rocket::get("/public/page")]
pub async fn get_public_page(
    public_service: &State<Arc<PublicPageService>>,
) -> AppResult<Json<SingleResponse<PublicPageResponse>>> {
    let page_response = public_service.get_home_page().await?;

    Ok(Json(SingleResponse {
        data: page_response,
    }))
}

/// Submit public testimonial
///
/// Public endpoint to submit a review/testimonial for moderation.
/// Always created with pending moderation status, unapproved and hidden. No authentication required.
#[utoipa::path(
    post,
    path = "/api/v1/public/testimonials",
    tag = "Public",
    request_body(content = SubmitPublicTestimonialRequest, description = "Public testimonial submission payload"),
    responses(
        (status = 201, description = "Testimonial submitted successfully for moderation", body = SingleResponse<SubmitPublicTestimonialResponse>),
        (status = 422, description = "Validation error", body = crate::shared::errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = crate::shared::errors::ApiErrorResponse)
    )
)]
#[rocket::post("/public/testimonials", data = "<req>")]
pub async fn submit_public_testimonial(
    req: Json<SubmitPublicTestimonialRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<(
    Status,
    Json<SingleResponse<SubmitPublicTestimonialResponse>>,
)> {
    let service = TestimonialService::new(db.inner(), (*storage).clone());
    let res = service.submit_public_testimonial(req.into_inner()).await?;
    Ok((Status::Created, Json(SingleResponse { data: res })))
}
