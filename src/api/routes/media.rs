use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminMediaDto, CreateYoutubeMediaRequest, MessageDataDto};
use crate::application::services::media_service::MediaService;
use crate::config::AppConfig;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

#[derive(rocket::FromForm)]
pub struct UploadForm<'f> {
    pub file: TempFile<'f>,
}

/// Upload media file
///
/// Uploads an image (JPEG, PNG, WebP) or video (MP4, WebM) file via multipart/form-data. File size limits and allowed MIME types are enforced by server configuration. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/media/upload",
    tag = "Media",
    request_body(content = UploadMediaRequest, content_type = "multipart/form-data", description = "Multipart form data with file field"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Media file uploaded successfully", body = SingleResponse<AdminMediaDto>),
        (status = 400, description = "Unsupported media format or payload too large", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/media/upload", data = "<form>")]
pub async fn upload_media(
    auth: AuthenticatedUser,
    mut form: Form<UploadForm<'_>>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminMediaDto>>> {
    let original_filename = form.file.name().map(|s| s.to_string());
    let content_type = form
        .file
        .content_type()
        .map(|ct| format!("{}/{}", ct.top(), ct.sub()))
        .unwrap_or_default();
    let file_size = form.file.len();

    let temp_path = match form.file.path() {
        Some(p) => p.to_path_buf(),
        None => {
            let temp_dir = std::env::temp_dir();
            let p = temp_dir.join(format!("upload_{}", Uuid::new_v4()));
            form.file.persist_to(&p).await.map_err(|e| {
                AppError::Internal(format!("Failed to persist temp upload file: {}", e))
            })?;
            p
        }
    };

    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    let dto = service
        .process_uploaded_file(
            &auth,
            &temp_path,
            original_filename.as_deref(),
            &content_type,
            file_size,
        )
        .await?;

    Ok(Json(SingleResponse { data: dto }))
}

/// Register YouTube media
///
/// Registers YouTube video metadata and reference URL without uploading a file. Generates embed and thumbnail URLs. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/media/youtube",
    tag = "Media",
    request_body(content = CreateYoutubeMediaRequest, description = "YouTube URL and metadata registration payload"),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "YouTube media registered successfully", body = SingleResponse<AdminMediaDto>),
        (status = 400, description = "Invalid YouTube URL format", body = ApiErrorResponse),
        (status = 422, description = "Validation error", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::post("/admin/media/youtube", data = "<req>")]
pub async fn create_youtube_media(
    auth: AuthenticatedUser,
    req: Json<CreateYoutubeMediaRequest>,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminMediaDto>>> {
    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    let dto = service
        .create_youtube_media(&auth, req.into_inner())
        .await?;
    Ok(Json(SingleResponse { data: dto }))
}

/// List media assets
///
/// Returns all uploaded file assets and registered YouTube media references. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/media",
    tag = "Media",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of media assets", body = SingleResponse<Vec<AdminMediaDto>>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/media")]
pub async fn list_media(
    auth: AuthenticatedUser,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<Vec<AdminMediaDto>>>> {
    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    let dtos = service.list_media(&auth).await?;
    Ok(Json(SingleResponse { data: dtos }))
}

/// Get media asset
///
/// Returns details of a single media asset by UUID identifier. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/media/{id}",
    tag = "Media",
    params(
        ("id" = Uuid, Path, description = "Media asset UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Media asset details", body = SingleResponse<AdminMediaDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Media asset not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::get("/admin/media/<id_str>")]
pub async fn get_media(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<AdminMediaDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid media ID".to_string(),
        }])
    })?;
    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    let dto = service.get_media(&auth, id).await?;
    Ok(Json(SingleResponse { data: dto }))
}

/// Delete media asset
///
/// Deletes an unused media asset by UUID identifier. If media is attached to content blocks, deletion is rejected with 409 Conflict. Requires authenticated super_admin or admin.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/media/{id}",
    tag = "Media",
    params(
        ("id" = Uuid, Path, description = "Media asset UUID identifier")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Media asset deleted successfully", body = SingleResponse<MessageDataDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Media asset not found", body = ApiErrorResponse),
        (status = 409, description = "Conflict - Media asset is currently in use", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[rocket::delete("/admin/media/<id_str>")]
pub async fn delete_media(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<SingleResponse<MessageDataDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid media ID".to_string(),
        }])
    })?;
    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    service.delete_media(&auth, id).await?;
    Ok(Json(SingleResponse {
        data: MessageDataDto {
            message: "Media asset deleted successfully".to_string(),
        },
    }))
}
