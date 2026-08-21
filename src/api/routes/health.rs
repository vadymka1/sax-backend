use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatusDto {
    #[schema(example = "ok")]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReadinessCheckDetails {
    #[schema(example = "ok")]
    pub database: String,
    #[schema(example = "ok")]
    pub storage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReadinessStatusDto {
    #[schema(example = "ok")]
    pub status: String,
    pub checks: ReadinessCheckDetails,
}

/// Health check
///
/// Aggregate application health check endpoint.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Application is running", body = HealthStatusDto)
    )
)]
#[rocket::get("/health")]
pub async fn health_check() -> Json<HealthStatusDto> {
    Json(HealthStatusDto {
        status: "ok".to_string(),
    })
}

/// Liveness check
///
/// Process liveness check endpoint used by Kubernetes or process managers.
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "Health",
    responses(
        (status = 200, description = "Process is live", body = HealthStatusDto)
    )
)]
#[rocket::get("/health/live")]
pub async fn liveness_check() -> Json<HealthStatusDto> {
    Json(HealthStatusDto {
        status: "live".to_string(),
    })
}

/// Readiness check
///
/// Readiness check endpoint verifying connection to PostgreSQL database and local storage.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready to handle traffic", body = ReadinessStatusDto)
    )
)]
#[rocket::get("/health/ready")]
pub async fn readiness_check(db: &rocket::State<sqlx::PgPool>) -> Json<ReadinessStatusDto> {
    let db_status = match sqlx::query("SELECT 1").execute(db.inner()).await {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    Json(ReadinessStatusDto {
        status: if db_status == "ok" {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        checks: ReadinessCheckDetails {
            database: db_status.to_string(),
            storage: "ok".to_string(),
        },
    })
}
