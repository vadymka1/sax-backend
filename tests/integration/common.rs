use rocket::local::asynchronous::Client;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use spa_sax_backend::bootstrap::build_rocket;
use spa_sax_backend::config::AppConfig;
use spa_sax_backend::domain::users::Role;
use spa_sax_backend::infrastructure::auth::{PasswordService, TokenService};

#[allow(dead_code)]
pub struct TestHarness {
    pub pool: PgPool,
    pub config: AppConfig,
    pub client: Client,
    pub temp_dir: tempfile::TempDir,
    pub super_admin_id: uuid::Uuid,
    pub super_admin_token: String,
    pub super_admin_email: String,
}

impl TestHarness {
    pub async fn new() -> Self {
        dotenvy::from_filename(".env.local").ok();
        dotenvy::dotenv().ok();

        let raw_db_url = std::env::var("DATABASE_URL").unwrap_or_else(|e| {
            panic!(
                "DATABASE_URL environment variable is required for integration tests: {}",
                e
            );
        });

        // Convert docker internal hostname 'postgres' to 'localhost' if running outside docker container
        let database_url = if raw_db_url.contains("@postgres:5432") {
            raw_db_url.replace("@postgres:5432", "@localhost:5432")
        } else {
            raw_db_url
        };

        let app_env = std::env::var("APP_ENV").unwrap_or_default();

        if let Err(err) =
            spa_sax_backend::config::validate_test_database_environment(&app_env, &database_url)
        {
            panic!("Database safety validation failed: {}", err);
        }

        let pool = match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(p) => p,
            Err(err) => {
                eprintln!(
                    "\n=== TEST HARNESS ERROR: Failed to connect to DB at {} -> {} ===",
                    database_url, err
                );
                panic!(
                    "Integration PostgreSQL database MUST be running at {}: {}",
                    database_url, err
                );
            }
        };

        // Run migrations
        if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
            eprintln!("\n=== TEST HARNESS MIGRATION ERROR: {} ===", err);
            panic!("SQLx migrations failed during test setup: {}", err);
        }

        let temp_dir = tempfile::tempdir().expect("Failed to create temporary uploads dir");

        let super_admin_id = uuid::Uuid::new_v4();
        let super_admin_email = format!("superadmin_{}@example.com", super_admin_id.simple());
        let super_admin_pass = "SuperAdminSecret123!";
        let pass_hash =
            PasswordService::hash_password(super_admin_pass).expect("Password hashing failed");

        // Seed super admin user fixture
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, display_name, role, is_active)
            VALUES ($1, $2, $3, 'Super Admin Test', 'super_admin', TRUE)
            "#,
        )
        .bind(super_admin_id)
        .bind(&super_admin_email)
        .bind(pass_hash)
        .execute(&pool)
        .await
        .expect("Failed to insert initial super admin test fixture");

        let jwt_secret = "test_jwt_access_secret_key_min_32_bytes_123456";
        let super_admin_token = TokenService::generate_access_token(
            super_admin_id,
            &super_admin_email,
            Role::SuperAdmin,
            jwt_secret,
            900,
        )
        .expect("JWT token generation failed");

        let mut config = AppConfig::from_env().unwrap_or_else(|_| AppConfig {
            env: "test".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8000,
            base_url: "http://localhost:8000".to_string(),
            database_url: database_url.clone(),
            database_max_connections: 5,
            jwt_access_secret: jwt_secret.to_string(),
            jwt_refresh_secret: "test_jwt_refresh_secret_key_min_32_bytes_123456".to_string(),
            jwt_access_ttl_seconds: 900,
            jwt_refresh_ttl_seconds: 2592000,
            password_min_length: 6,
            cors_allowed_origins: vec![
                "http://localhost:5173".to_string(),
                "http://localhost:3000".to_string(),
            ],
            storage_provider: "local".to_string(),
            local_storage_path: temp_dir.path().to_string_lossy().to_string(),
            public_media_base_url: "http://localhost:8000/uploads".to_string(),
            max_image_upload_mb: 10,
            max_video_upload_mb: 100,
            smtp: spa_sax_backend::config::SmtpConfig {
                enabled: false,
                host: "".to_string(),
                port: 587,
                username: None,
                password: None,
                from_email: "test@example.com".to_string(),
                from_name: "Test".to_string(),
                contact_notification_email: "test@example.com".to_string(),
                starttls: false,
            },
        });
        config.database_url = database_url;
        config.jwt_access_secret = jwt_secret.to_string();
        config.password_min_length = 6;
        config.local_storage_path = temp_dir.path().to_string_lossy().to_string();

        let rocket = build_rocket(config.clone())
            .await
            .expect("Rocket build failed in test harness");

        let client = Client::tracked(rocket)
            .await
            .expect("Rocket local client initialization failed");

        Self {
            pool,
            config,
            client,
            temp_dir,
            super_admin_id,
            super_admin_token,
            super_admin_email,
        }
    }
}

pub async fn cleanup_test_page(pool: &PgPool, page_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM section_media WHERE section_id IN (SELECT id FROM sections WHERE page_id = $1 OR spa_section_id IN (SELECT id FROM spa_sections WHERE page_id = $1))",
    )
    .bind(page_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM sections WHERE page_id = $1 OR spa_section_id IN (SELECT id FROM spa_sections WHERE page_id = $1)")
        .bind(page_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM spa_sections WHERE page_id = $1")
        .bind(page_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(page_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn cleanup_content_block(pool: &PgPool, block_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM section_media WHERE section_id = $1")
        .bind(block_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(block_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn cleanup_spa_section(pool: &PgPool, section_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM section_media WHERE section_id IN (SELECT id FROM sections WHERE spa_section_id = $1)",
    )
    .bind(section_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM sections WHERE spa_section_id = $1")
        .bind(section_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM spa_sections WHERE id = $1")
        .bind(section_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn reset_home_sections_to_bootstrap(pool: &PgPool) {
    let canonical_ids = "('11111111-1111-1111-1111-111111111111', '22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333333', '44444444-4444-4444-4444-444444444444', '55555555-5555-5555-5555-555555555555', '66666666-6666-6666-6666-666666666666')";

    sqlx::query(&format!(
        "DELETE FROM section_media WHERE section_id IN (SELECT id FROM sections WHERE spa_section_id NOT IN {})",
        canonical_ids
    ))
    .execute(pool)
    .await
    .ok();

    sqlx::query(&format!(
        "DELETE FROM sections WHERE spa_section_id NOT IN {}",
        canonical_ids
    ))
    .execute(pool)
    .await
    .ok();

    sqlx::query(&format!(
        "DELETE FROM spa_sections WHERE page_id = (SELECT id FROM pages WHERE slug = 'home') AND id NOT IN {}",
        canonical_ids
    ))
    .execute(pool)
    .await
    .ok();

    // Ensure testimonials exists
    sqlx::query(
        "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
         SELECT '66666666-6666-6666-6666-666666666666'::uuid, id, 'testimonials', 'Testimonials', 'Testimonials', 40, TRUE
         FROM pages WHERE slug = 'home'
         ON CONFLICT (page_id, section_key) DO UPDATE
         SET sort_order = 40, is_visible = TRUE, deleted_at = NULL"
    )
    .execute(pool)
    .await
    .ok();

    // Ensure 4 canonical sections are visible with exact sort order
    sqlx::query("UPDATE spa_sections SET sort_order = 10, is_visible = TRUE, deleted_at = NULL WHERE id = '11111111-1111-1111-1111-111111111111'").execute(pool).await.ok();
    sqlx::query("UPDATE spa_sections SET sort_order = 20, is_visible = TRUE, deleted_at = NULL WHERE id = '44444444-4444-4444-4444-444444444444'").execute(pool).await.ok();
    sqlx::query("UPDATE spa_sections SET sort_order = 30, is_visible = TRUE, deleted_at = NULL WHERE id = '55555555-5555-5555-5555-555555555555'").execute(pool).await.ok();
    sqlx::query("UPDATE spa_sections SET sort_order = 40, is_visible = TRUE, deleted_at = NULL WHERE id = '66666666-6666-6666-6666-666666666666'").execute(pool).await.ok();

    // Ensure old defaults are hidden
    sqlx::query("UPDATE spa_sections SET sort_order = 50, is_visible = FALSE, deleted_at = NULL WHERE id = '22222222-2222-2222-2222-222222222222'").execute(pool).await.ok();
    sqlx::query("UPDATE spa_sections SET sort_order = 60, is_visible = FALSE, deleted_at = NULL WHERE id = '33333333-3333-3333-3333-333333333333'").execute(pool).await.ok();
}

pub async fn cleanup_test_user(pool: &PgPool, user_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn cleanup_media(pool: &PgPool, media_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM section_media WHERE media_asset_id = $1")
        .bind(media_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM media_assets WHERE id = $1")
        .bind(media_id)
        .execute(pool)
        .await?;

    Ok(())
}
