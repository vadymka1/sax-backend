use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminMediaDto, CreateYoutubeMediaRequest};
use crate::application::services::media_service::MediaService;
use crate::config::AppConfig;
use crate::infrastructure::storage::StorageProvider;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

#[derive(rocket::FromForm)]
pub struct UploadForm<'f> {
    pub file: TempFile<'f>,
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/media/upload",
    responses((status = 200, body = SingleResponse<AdminMediaDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/media/youtube",
    request_body = CreateYoutubeMediaRequest,
    responses((status = 200, body = SingleResponse<AdminMediaDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/media",
    responses((status = 200, body = SingleResponse<Vec<AdminMediaDto>>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/media/{id}",
    responses((status = 200, body = SingleResponse<AdminMediaDto>)),
    security(("bearer_auth" = []))
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

#[utoipa::path(
    delete,
    path = "/api/v1/admin/media/{id}",
    responses((status = 200)),
    security(("bearer_auth" = []))
)]
#[rocket::delete("/admin/media/<id_str>")]
pub async fn delete_media(
    auth: AuthenticatedUser,
    id_str: &str,
    db: &State<PgPool>,
    config: &State<AppConfig>,
    storage: &State<Arc<dyn StorageProvider>>,
) -> AppResult<Json<serde_json::Value>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid media ID".to_string(),
        }])
    })?;
    let service = MediaService::new(db.inner(), config.inner(), storage.inner().clone());
    service.delete_media(&auth, id).await?;
    Ok(Json(
        serde_json::json!({ "data": { "message": "Media asset deleted successfully" } }),
    ))
}
