use std::sync::Arc;

use rocket::serde::json::Json;
use rocket::State;

use crate::application::dto::PublicPageResponse;
use crate::application::services::PublicPageService;
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
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
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
