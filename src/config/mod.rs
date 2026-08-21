use crate::shared::errors::{AppError, AppResult};
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub env: String,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub jwt_access_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_access_ttl_seconds: i64,
    pub jwt_refresh_ttl_seconds: i64,
    pub password_min_length: usize,
    pub cors_allowed_origins: Vec<String>,
    pub storage_provider: String,
    pub local_storage_path: String,
    pub public_media_base_url: String,
    pub max_image_upload_mb: u64,
    pub max_video_upload_mb: u64,
}

impl AppConfig {
    pub fn from_env() -> AppResult<Self> {
        dotenvy::dotenv().ok();

        let env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse::<u16>()
            .map_err(|_| AppError::Internal("Invalid APP_PORT".to_string()))?;
        let base_url =
            env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

        let database_url = env::var("DATABASE_URL").map_err(|_| {
            AppError::Internal("DATABASE_URL environment variable is required".to_string())
        })?;
        let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .map_err(|_| AppError::Internal("Invalid DATABASE_MAX_CONNECTIONS".to_string()))?;

        let jwt_access_secret = env::var("JWT_ACCESS_SECRET").map_err(|_| {
            AppError::Internal("JWT_ACCESS_SECRET environment variable is required".to_string())
        })?;
        let jwt_refresh_secret = env::var("JWT_REFRESH_SECRET").map_err(|_| {
            AppError::Internal("JWT_REFRESH_SECRET environment variable is required".to_string())
        })?;
        let jwt_access_ttl_seconds = env::var("JWT_ACCESS_TTL_SECONDS")
            .unwrap_or_else(|_| "900".to_string())
            .parse::<i64>()
            .map_err(|_| AppError::Internal("Invalid JWT_ACCESS_TTL_SECONDS".to_string()))?;
        let jwt_refresh_ttl_seconds = env::var("JWT_REFRESH_TTL_SECONDS")
            .unwrap_or_else(|_| "2592000".to_string())
            .parse::<i64>()
            .map_err(|_| AppError::Internal("Invalid JWT_REFRESH_TTL_SECONDS".to_string()))?;

        let password_min_length = env::var("PASSWORD_MIN_LENGTH")
            .unwrap_or_else(|_| "6".to_string())
            .parse::<usize>()
            .map_err(|_| AppError::Internal("Invalid PASSWORD_MIN_LENGTH".to_string()))?;

        let cors_raw = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://localhost:5173".to_string());
        let cors_allowed_origins = cors_raw.split(',').map(|s| s.trim().to_string()).collect();

        let storage_provider = env::var("STORAGE_PROVIDER").unwrap_or_else(|_| "local".to_string());
        let local_storage_path =
            env::var("LOCAL_STORAGE_PATH").unwrap_or_else(|_| "./uploads".to_string());
        let public_media_base_url = env::var("PUBLIC_MEDIA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8000/uploads".to_string());

        let max_image_upload_mb = env::var("MAX_IMAGE_UPLOAD_MB")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u64>()
            .map_err(|_| AppError::Internal("Invalid MAX_IMAGE_UPLOAD_MB".to_string()))?;
        let max_video_upload_mb = env::var("MAX_VIDEO_UPLOAD_MB")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u64>()
            .map_err(|_| AppError::Internal("Invalid MAX_VIDEO_UPLOAD_MB".to_string()))?;

        Ok(Self {
            env,
            host,
            port,
            base_url,
            database_url,
            database_max_connections,
            jwt_access_secret,
            jwt_refresh_secret,
            jwt_access_ttl_seconds,
            jwt_refresh_ttl_seconds,
            password_min_length,
            cors_allowed_origins,
            storage_provider,
            local_storage_path,
            public_media_base_url,
            max_image_upload_mb,
            max_video_upload_mb,
        })
    }
}

pub fn validate_test_database_environment(app_env: &str, database_url: &str) -> Result<(), String> {
    if app_env != "test" {
        return Err(format!(
            "Integration tests require APP_ENV=test, but got '{}'",
            app_env
        ));
    }

    let url = url::Url::parse(database_url).map_err(|e| format!("Invalid DATABASE_URL: {}", e))?;
    let path = url.path().trim_start_matches('/');
    let db_name = path.split('?').next().unwrap_or("").trim();

    if db_name.is_empty() {
        return Err("DATABASE_URL missing database name".to_string());
    }

    let is_test_db = db_name.ends_with("_test")
        || db_name.starts_with("test_")
        || db_name == "integration_test"
        || db_name == "app_test"
        || db_name == "spa_backend_test";

    if !is_test_db {
        return Err(format!(
            "Database name '{}' is not an explicitly approved test database name",
            db_name
        ));
    }

    Ok(())
}
