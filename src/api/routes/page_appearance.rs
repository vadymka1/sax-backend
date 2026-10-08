use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use std::sync::Arc;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminPageAppearanceDto, UpdatePageAppearanceRequest};
use crate::application::services::PageAppearanceService;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::AppResult;
use crate::shared::pagination::SingleResponse;

/// Get page appearance settings
///
/// Retrieves the appearance configuration for the public SPA home page, including optional background media asset, overlay opacity, position, and size. Requires admin authentication.
#[utoipa::path(
    get,
    path = "/api/v1/admin/page-appearance",
    tag = "Page Appearance",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Page appearance settings", body = SingleResponse<AdminPageAppearanceDto>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/page-appearance")]
pub async fn get_page_appearance(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminPageAppearanceDto>>> {
    let service = PageAppearanceService::new(db.inner(), storage.inner().clone());
    let appearance = service.get_appearance(&auth).await?;
    Ok(Json(SingleResponse { data: appearance }))
}

/// Alias for GET /api/v1/admin/pages/home/appearance
#[rocket::get("/admin/pages/home/appearance", rank = 2)]
pub async fn get_home_page_appearance_alias(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminPageAppearanceDto>>> {
    get_page_appearance(auth, db, storage).await
}

/// Update page appearance settings
///
/// Partially updates the appearance configuration for the public SPA home page.
/// Supports assigning background image (`background_media_id`), detaching background image (`background_media_id: null`),
/// adjusting overlay opacity (0.0 to 1.0), and configuring position ("center" | "top" | "bottom") and size ("cover" | "contain").
/// Background media MUST be an active image asset (videos and youtube references are rejected).
#[utoipa::path(
    patch,
    path = "/api/v1/admin/page-appearance",
    tag = "Page Appearance",
    request_body(content = UpdatePageAppearanceRequest, description = "Page appearance partial update payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Updated page appearance settings", body = SingleResponse<AdminPageAppearanceDto>),
        (status = 400, description = "Bad request", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Referenced media asset not found", body = ApiErrorResponse),
        (status = 422, description = "Validation error (e.g. invalid opacity or non-image media)", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::patch("/admin/page-appearance", data = "<input>")]
pub async fn update_page_appearance(
    auth: AuthenticatedUser,
    input: Json<UpdatePageAppearanceRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminPageAppearanceDto>>> {
    let service = PageAppearanceService::new(db.inner(), storage.inner().clone());
    let appearance = service.update_appearance(&auth, input.into_inner()).await?;
    Ok(Json(SingleResponse { data: appearance }))
}

/// Alias for PATCH /api/v1/admin/pages/home/appearance
#[rocket::patch("/admin/pages/home/appearance", data = "<input>", rank = 2)]
pub async fn update_home_page_appearance_alias(
    auth: AuthenticatedUser,
    input: Json<UpdatePageAppearanceRequest>,
    db: &State<PgPool>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminPageAppearanceDto>>> {
    update_page_appearance(auth, input, db, storage).await
}
