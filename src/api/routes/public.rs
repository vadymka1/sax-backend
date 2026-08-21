use std::sync::Arc;

use rocket::serde::json::Json;
use rocket::State;

use crate::application::dto::PublicPageResponse;
use crate::application::services::PublicPageService;
use crate::shared::errors::AppResult;
use crate::shared::pagination::SingleResponse;

#[utoipa::path(
    get,
    path = "/api/v1/public/page",
    responses((status = 200, body = SingleResponse<PublicPageResponse>))
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
