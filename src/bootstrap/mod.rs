use std::sync::Arc;

use rocket::fs::FileServer;
use rocket::response::Redirect;
use rocket::{catchers, routes, Build, Rocket};
use sqlx::postgres::PgPoolOptions;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

use crate::api::catchers;
use crate::api::fairings::SecurityHeadersFairing;
use crate::api::routes;
use crate::application::dto;
use crate::config::AppConfig;
use crate::infrastructure::storage::{LocalStorageProvider, StorageProvider};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

#[rocket::get("/swagger-ui", rank = 10)]
fn swagger_ui_redirect() -> Redirect {
    Redirect::to("/swagger-ui/")
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "SPA Sax Backend API",
        description = "Backend API for the SPA Sax public website and protected administration interface. Supports public aggregated page query, JWT Bearer authentication, SPA sections management, home page content blocks CRUD, local file uploads & YouTube media references, super-admin user management, and health checks."
    ),
    paths(
        routes::health::health_check,
        routes::health::liveness_check,
        routes::health::readiness_check,
        routes::auth::login,
        routes::auth::refresh,
        routes::auth::logout,
        routes::auth::get_me,
        routes::users::list_users,
        routes::users::create_user,
        routes::users::update_user,
        routes::users::activate_user,
        routes::users::deactivate_user,
        routes::content_blocks::list_content_blocks,
        routes::content_blocks::get_content_block,
        routes::content_blocks::create_content_block,
        routes::content_blocks::update_content_block,
        routes::content_blocks::delete_content_block,
        routes::spa_sections::list_spa_sections,
        routes::spa_sections::create_spa_section,
        routes::spa_sections::get_spa_section,
        routes::spa_sections::update_spa_section,
        routes::spa_sections::delete_spa_section,
        routes::spa_sections::reorder_spa_sections,
        routes::spa_sections::reorder_section_content_blocks,
        routes::media::upload_media,
        routes::media::create_youtube_media,
        routes::media::list_media,
        routes::media::get_media,
        routes::media::delete_media,
        routes::public::get_public_page,
        routes::contact::submit_contact_form,
    ),
    components(
        schemas(
            dto::LoginRequest,
            dto::RefreshTokenRequest,
            dto::LogoutRequest,
            dto::AuthTokensDto,
            dto::RefreshTokenDataDto,
            dto::MessageDataDto,
            dto::UserDto,
            dto::CreateUserRequest,
            dto::UpdateUserRequest,
            dto::AdminSpaSectionDto,
            dto::CreateSpaSectionRequest,
            dto::UpdateSpaSectionRequest,
            dto::ReorderSpaSectionItem,
            dto::ReorderSpaSectionsRequest,
            dto::AdminContentBlockDto,
            dto::CreateContentBlockRequest,
            dto::UpdateContentBlockRequest,
            dto::ReorderContentBlockItem,
            dto::ReorderContentBlocksRequest,
            dto::BlockAttachedMediaDto,
            dto::MediaResponseDto,
            dto::AdminMediaDto,
            dto::CreateYoutubeMediaRequest,
            dto::YoutubeMediaResponseDto,
            dto::UploadMediaRequest,
            dto::PublicPageResponse,
            dto::PublicPageDto,
            dto::PublicSpaSectionDto,
            dto::PublicContentBlockDto,
            dto::PublicMediaDto,
            dto::ContactRequest,
            dto::ContactResponse,
            routes::health::HealthStatusDto,
            routes::health::ReadinessStatusDto,
            routes::health::ReadinessCheckDetails,
            crate::domain::users::Role,
            crate::domain::sections::ContentBlockType,
            crate::domain::sections::FontFamily,
            crate::domain::sections::FontSize,
            crate::shared::pagination::PaginationMeta,
            crate::shared::errors::ApiErrorPayload,
            crate::shared::errors::ApiErrorDetails,
            crate::shared::errors::ApiErrorResponse
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "Auth", description = "Authentication & Token Management"),
        (name = "Admin Users", description = "Admin User Management API"),
        (name = "SPA Sections", description = "Dynamic SPA Sections Management"),
        (name = "Content Blocks", description = "Home Page Content Blocks CRUD"),
        (name = "Media", description = "Local File Uploads & YouTube Media"),
        (name = "Public", description = "Public Aggregated Website API"),
        (name = "Health", description = "Service Health Checks")
    )
)]
pub struct ApiDoc;

pub async fn build_rocket(config: AppConfig) -> Result<Rocket<Build>, Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await?;

    let storage: Arc<dyn StorageProvider> = Arc::new(LocalStorageProvider::new(
        &config.local_storage_path,
        config.public_media_base_url.clone(),
    ));

    let public_page_service = Arc::new(crate::application::services::PublicPageService::new(
        pool.clone(),
        storage.clone(),
    ));

    let contact_mailer: crate::application::services::DynContactMailer = if config.smtp.enabled {
        Arc::new(crate::application::services::SmtpContactMailer::new(
            config.smtp.clone(),
        ))
    } else {
        Arc::new(crate::application::services::NoopContactMailer)
    };

    let contact_service = crate::application::services::ContactService::new(
        pool.clone(),
        contact_mailer,
        config.smtp.clone(),
    );

    let figment = rocket::Config::figment()
        .merge(("address", config.host.parse::<std::net::IpAddr>()?))
        .merge(("port", config.port));

    let rocket = rocket::custom(figment)
        .manage(config.clone())
        .manage(pool)
        .manage(storage)
        .manage(public_page_service)
        .manage(contact_service)
        .attach(crate::api::fairings::CorsFairing)
        .attach(SecurityHeadersFairing)
        .register(
            "/",
            catchers![catchers::not_found, catchers::internal_error],
        )
        .mount(
            "/api/v1",
            routes![
                routes::auth::login,
                routes::auth::refresh,
                routes::auth::logout,
                routes::auth::get_me,
                routes::users::list_users,
                routes::users::create_user,
                routes::users::update_user,
                routes::users::activate_user,
                routes::users::deactivate_user,
                routes::spa_sections::list_spa_sections,
                routes::spa_sections::create_spa_section,
                routes::spa_sections::get_spa_section,
                routes::spa_sections::update_spa_section,
                routes::spa_sections::delete_spa_section,
                routes::spa_sections::reorder_spa_sections,
                routes::spa_sections::reorder_section_content_blocks,
                routes::content_blocks::list_content_blocks,
                routes::content_blocks::get_content_block,
                routes::content_blocks::create_content_block,
                routes::content_blocks::update_content_block,
                routes::content_blocks::delete_content_block,
                routes::media::upload_media,
                routes::media::create_youtube_media,
                routes::media::list_media,
                routes::media::get_media,
                routes::media::delete_media,
                routes::public::get_public_page,
                routes::contact::submit_contact_form,
            ],
        )
        .mount(
            "/",
            routes![
                routes::health::health_check,
                routes::health::liveness_check,
                routes::health::readiness_check,
            ],
        )
        .mount("/uploads", FileServer::from(&config.local_storage_path));

    Ok(mount_swagger(rocket, &config.env))
}

pub fn mount_swagger(rocket: Rocket<Build>, env: &str) -> Rocket<Build> {
    if env == "development" {
        rocket.mount("/", routes![swagger_ui_redirect]).mount(
            "/",
            SwaggerUi::new("/swagger-ui/<_..>").url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
    } else {
        rocket
    }
}
