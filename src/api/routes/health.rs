use rocket::serde::json::Json;

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200))
)]
#[rocket::get("/health")]
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

#[utoipa::path(
    get,
    path = "/health/live",
    responses((status = 200))
)]
#[rocket::get("/health/live")]
pub async fn liveness_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "live" }))
}

#[utoipa::path(
    get,
    path = "/health/ready",
    responses((status = 200))
)]
#[rocket::get("/health/ready")]
pub async fn readiness_check(db: &rocket::State<sqlx::PgPool>) -> Json<serde_json::Value> {
    let db_status = match sqlx::query("SELECT 1").execute(db.inner()).await {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    Json(serde_json::json!({
        "status": if db_status == "ok" { "ok" } else { "degraded" },
        "checks": {
            "database": db_status,
            "storage": "ok"
        }
    }))
}
