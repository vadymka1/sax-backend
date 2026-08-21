use rocket::http::{Header, Method, Status};
use serde_json::json;

use argon2::PasswordHasher;
use uuid::Uuid;

use spa_sax_backend::application::dto::{
    AdminContentBlockDto, AdminMediaDto, AdminSpaSectionDto, PublicMediaDto, PublicPageResponse,
    UserDto,
};
use spa_sax_backend::domain::users::Role;
use spa_sax_backend::infrastructure::auth::PasswordService;
use spa_sax_backend::shared::pagination::SingleResponse;

mod common;
use common::TestHarness;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_home_page_migration_exists() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    common::reset_home_sections_to_bootstrap(&harness.pool).await;
    let page: (uuid::Uuid, String) =
        sqlx::query_as("SELECT id, slug FROM pages WHERE slug = 'home'")
            .fetch_one(&harness.pool)
            .await
            .expect("Home page MUST exist in test database");

    assert_eq!(page.1, "home");

    // 1. Verify 5 default SPA sections exist for home page in deterministic order
    let repo = spa_sax_backend::infrastructure::repositories::spa_section_repository::SpaSectionRepository::new(&harness.pool);
    let spa_sections = repo
        .list_for_page(page.0)
        .await
        .expect("Failed to list SPA sections");
    assert_eq!(spa_sections.len(), 5);

    let keys: Vec<&str> = spa_sections
        .iter()
        .map(|s| s.section_key.as_str())
        .collect();
    assert_eq!(
        keys,
        vec![
            "about-us",
            "our-works",
            "festivals",
            "gallery",
            "contact-us"
        ]
    );

    let orders: Vec<i32> = spa_sections.iter().map(|s| s.sort_order).collect();
    assert_eq!(orders, vec![10, 20, 30, 40, 50]);

    // 2. Verify find_by_key works
    let about_sec = repo
        .find_by_key(page.0, "about-us")
        .await
        .expect("Failed to query about-us section");
    assert!(about_sec.is_some());
    assert_eq!(about_sec.unwrap().title, "About Us");

    // 4. Verify NOT NULL invariant: count of sections with NULL spa_section_id is exactly 0
    let null_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sections WHERE spa_section_id IS NULL")
            .fetch_one(&harness.pool)
            .await
            .expect("Failed to count null spa_section_id");
    assert_eq!(
        null_count.0, 0,
        "No content blocks can have NULL spa_section_id"
    );

    // 5. Verify information_schema.columns reports is_nullable = 'NO' for sections.spa_section_id
    let col_nullable: (String,) = sqlx::query_as(
        "SELECT is_nullable FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'sections' AND column_name = 'spa_section_id'"
    )
    .fetch_one(&harness.pool)
    .await
    .expect("Failed to query information_schema for sections.spa_section_id");
    assert_eq!(
        col_nullable.0, "NO",
        "sections.spa_section_id MUST be NOT NULL in schema metadata"
    );

    // 6. Verify PostgreSQL CHECK constraint rejects invalid section_key formats on an isolated test page
    let test_page_id = uuid::Uuid::new_v4();
    let test_slug = format!("test-page-{}", test_page_id.simple());
    sqlx::query("INSERT INTO pages (id, slug, title) VALUES ($1, $2, 'Isolated Test Page')")
        .bind(test_page_id)
        .bind(&test_slug)
        .execute(&harness.pool)
        .await
        .expect("Failed to insert isolated test page");

    let invalid_keys = vec!["About Us", "about--us", "-", "-about", "about-"];
    for invalid_key in invalid_keys {
        let db_check_res = sqlx::query(
            "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label) VALUES ($1, $2, $3, 'Title', 'Nav')"
        )
        .bind(uuid::Uuid::new_v4())
        .bind(test_page_id)
        .bind(invalid_key)
        .execute(&harness.pool)
        .await;

        assert!(
            db_check_res.is_err(),
            "PostgreSQL CHECK constraint MUST reject invalid section_key: {}",
            invalid_key
        );
    }

    // 7. Verify valid section_key formats are accepted by PostgreSQL on isolated test page
    let valid_keys = vec!["about", "about-us", "festival-2026", "works2"];
    for valid_key in valid_keys {
        let valid_res = sqlx::query(
            "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label) VALUES ($1, $2, $3, 'Title', 'Nav')"
        )
        .bind(uuid::Uuid::new_v4())
        .bind(test_page_id)
        .bind(valid_key)
        .execute(&harness.pool)
        .await;

        assert!(
            valid_res.is_ok(),
            "PostgreSQL CHECK constraint MUST accept valid section_key: {}",
            valid_key
        );
    }

    // 8. Verify same-page composite ownership rejection: creating a section on page A with a spa_section_id from page B MUST fail
    let other_page_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO pages (id, slug, title) VALUES ($1, $2, 'Other Page')")
        .bind(other_page_id)
        .bind(format!("page_{}", other_page_id.simple()))
        .execute(&harness.pool)
        .await
        .expect("Failed to insert dummy other page");

    let other_spa_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order) VALUES ($1, $2, 'other-sec', 'Other', 'Other', 10)"
    )
    .bind(other_spa_id)
    .bind(other_page_id)
    .execute(&harness.pool)
    .await
    .expect("Failed to insert other page spa section");

    // Attempting to attach home content block to other_spa_id must fail at DB composite FK constraint
    let same_page_fk_res = sqlx::query(
        "INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, content) VALUES ($1, $2, $3, $4, 'text', '{}'::jsonb)"
    )
    .bind(uuid::Uuid::new_v4())
    .bind(page.0) // home page_id
    .bind(other_spa_id) // spa_section_id from other_page_id
    .bind(format!("key_{}", uuid::Uuid::new_v4().simple()))
    .execute(&harness.pool)
    .await;

    assert!(
        same_page_fk_res.is_err(),
        "Composite same-page foreign key MUST reject cross-page spa_section_id ownership"
    );

    if let Err(ref e) = common::cleanup_test_page(&harness.pool, test_page_id).await {
        panic!("Cleanup test_page failed with error: {}", e);
    }
    if let Err(e) = common::cleanup_test_page(&harness.pool, other_page_id).await {
        panic!("Cleanup other_page failed: {:?}", e);
    }
}

#[tokio::test]
async fn test_authentication_login_and_refresh_flow() {
    let harness = TestHarness::new().await;

    // 1. Create a user with password
    let email = format!("user_{}@example.com", uuid::Uuid::new_v4().simple());
    let password = "SecretPassword123!";
    let pass_hash = PasswordService::hash_password(password).unwrap();
    let user_id = uuid::Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, role, is_active) VALUES ($1, $2, $3, 'Test User', 'admin', TRUE)"
    )
    .bind(user_id)
    .bind(&email)
    .bind(pass_hash)
    .execute(&harness.pool)
    .await
    .unwrap();

    // 2. Perform HTTP Login
    let req = harness.client.post("/api/v1/auth/login").json(&json!({
        "email": email,
        "password": password
    }));
    let res = req.dispatch().await;
    assert_eq!(res.status(), Status::Ok);

    let body: serde_json::Value = res.into_json().await.unwrap();
    let access_token = body["data"]["access_token"].as_str().unwrap();
    let refresh_token = body["data"]["refresh_token"].as_str().unwrap();
    assert!(!access_token.is_empty());
    assert_eq!(refresh_token.len(), 64);

    // 3. Perform Refresh Token Rotation
    let refresh_req = harness
        .client
        .post("/api/v1/auth/refresh")
        .json(&json!({ "refresh_token": refresh_token }));
    let refresh_res = refresh_req.dispatch().await;
    assert_eq!(refresh_res.status(), Status::Ok);

    let refresh_body: serde_json::Value = refresh_res.into_json().await.unwrap();
    let new_refresh_token = refresh_body["data"]["refresh_token"].as_str().unwrap();
    assert_ne!(refresh_token, new_refresh_token);

    // 4. Logout revokes token
    let logout_req = harness
        .client
        .post("/api/v1/auth/logout")
        .json(&json!({ "refresh_token": new_refresh_token }));
    let logout_res = logout_req.dispatch().await;
    assert_eq!(logout_res.status(), Status::Ok);

    // Reuse revoked refresh token must fail
    let reuse_req = harness
        .client
        .post("/api/v1/auth/refresh")
        .json(&json!({ "refresh_token": new_refresh_token }));
    let reuse_res = reuse_req.dispatch().await;
    assert_eq!(reuse_res.status(), Status::Unauthorized);
}

#[tokio::test]
async fn test_admin_creation_permissions_and_duplicate_email() {
    let harness = TestHarness::new().await;

    // 1. Super Admin creates a new Admin via HTTP POST /api/v1/admin/users
    let admin_email = format!("new_admin_{}@example.com", uuid::Uuid::new_v4().simple());
    let req = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": admin_email,
            "display_name": "New Admin User",
            "password": "AdminPassword123!"
        }));

    let res = req.dispatch().await;
    assert_eq!(res.status(), Status::Ok);

    let body: serde_json::Value = res.into_json().await.unwrap();
    assert_eq!(body["data"]["email"], admin_email);
    assert_eq!(body["data"]["role"], "admin");

    // 2. Duplicate email creation must return HTTP 409 DUPLICATE_EMAIL
    let dup_req = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": admin_email.to_uppercase(),
            "display_name": "Duplicate Admin",
            "password": "AdminPassword123!"
        }));
    let dup_res = dup_req.dispatch().await;
    assert_eq!(dup_res.status(), Status::Conflict);

    let dup_json: serde_json::Value = dup_res.into_json().await.unwrap();
    assert_eq!(dup_json["error"]["code"], "DUPLICATE_EMAIL");
}

#[tokio::test]
async fn test_one_media_per_block_constraint_database_integrity() {
    let harness = TestHarness::new().await;

    let section_id = uuid::Uuid::new_v4();
    let media1_id = uuid::Uuid::new_v4();
    let media2_id = uuid::Uuid::new_v4();
    let page_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
        .fetch_one(&harness.pool)
        .await
        .expect("Home page missing");

    let spa_sec: (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM spa_sections WHERE page_id = $1 AND section_key = 'about-us'",
    )
    .bind(page_id.0)
    .fetch_one(&harness.pool)
    .await
    .expect("About us spa section missing");

    // Insert dummy section with valid spa_section_id
    sqlx::query(
        "INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content) VALUES ($1, $2, $3, $4, 'text', 'Test', '{}')",
    )
    .bind(section_id)
    .bind(page_id.0)
    .bind(spa_sec.0)
    .bind(format!("key_{}", section_id.simple()))
    .execute(&harness.pool)
    .await
    .expect("Section insert failed");

    // Insert dummy media assets
    sqlx::query("INSERT INTO media_assets (id, media_type, storage_provider) VALUES ($1, 'image', 'local'), ($2, 'image', 'local')")
        .bind(media1_id)
        .bind(media2_id)
        .execute(&harness.pool)
        .await
        .expect("Media assets insert failed");

    // First relation succeeds
    sqlx::query("INSERT INTO section_media (id, section_id, media_asset_id, usage_type) VALUES ($1, $2, $3, 'content')")
        .bind(uuid::Uuid::new_v4())
        .bind(section_id)
        .bind(media1_id)
        .execute(&harness.pool)
        .await
        .expect("First section_media insert failed");

    // Second relation on same section_id MUST fail with unique constraint violation (23505)
    let res = sqlx::query("INSERT INTO section_media (id, section_id, media_asset_id, usage_type) VALUES ($1, $2, $3, 'content')")
        .bind(uuid::Uuid::new_v4())
        .bind(section_id)
        .bind(media2_id)
        .execute(&harness.pool)
        .await;

    assert!(res.is_err());
    if let Err(sqlx::Error::Database(db_err)) = res {
        assert_eq!(db_err.code().as_deref(), Some("23505"));
    } else {
        panic!("Expected database unique constraint error code 23505");
    }

    // Clean up test fixtures
    sqlx::query("DELETE FROM section_media WHERE section_id = $1")
        .bind(section_id)
        .execute(&harness.pool)
        .await
        .ok();
    sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(section_id)
        .execute(&harness.pool)
        .await
        .ok();
    sqlx::query("DELETE FROM media_assets WHERE id IN ($1, $2)")
        .bind(media1_id)
        .bind(media2_id)
        .execute(&harness.pool)
        .await
        .ok();
}

#[tokio::test]
async fn test_cors_fairing_browser_preflight_and_origin_matching() {
    let harness = TestHarness::new().await;

    // 1. Valid origin http://localhost:5173 on /api/v1/admin/content-blocks OPTIONS preflight
    let req = harness
        .client
        .req(Method::Options, "/api/v1/admin/content-blocks")
        .header(Header::new("Origin", "http://localhost:5173"))
        .header(Header::new("Access-Control-Request-Method", "PATCH"))
        .header(Header::new(
            "Access-Control-Request-Headers",
            "Authorization, Content-Type",
        ));

    let res = req.dispatch().await;
    assert_eq!(res.status(), Status::NoContent);
    assert_eq!(
        res.headers().get_one("Access-Control-Allow-Origin"),
        Some("http://localhost:5173")
    );
    assert_eq!(res.headers().get_one("Vary"), Some("Origin"));

    // 2. Malicious lookalike origin http://localhost.evil.example preflight must return 403 Forbidden
    let req_evil = harness
        .client
        .req(Method::Options, "/api/v1/admin/content-blocks")
        .header(Header::new("Origin", "http://localhost.evil.example"));
    let res_evil = req_evil.dispatch().await;
    assert_eq!(res_evil.status(), Status::Forbidden);
    assert_eq!(
        res_evil.headers().get_one("Access-Control-Allow-Origin"),
        None
    );
}

#[tokio::test]
async fn test_admin_user_deactivation_protections_and_permissions() {
    let harness = TestHarness::new().await;

    let service = spa_sax_backend::application::services::admin_user_service::AdminUserService::new(
        &harness.pool,
        &harness.config,
    );

    let auth_super = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };

    // Self-Deactivation Protection
    let self_deactivate_res = service
        .toggle_active(&auth_super, harness.super_admin_id, false)
        .await;
    assert!(self_deactivate_res.is_err());
    if let Err(spa_sax_backend::shared::errors::AppError::ResourceConflict(msg)) =
        self_deactivate_res
    {
        assert!(msg.contains("cannot deactivate your own account"));
    } else {
        panic!("Expected ResourceConflict error on self deactivation");
    }
}

#[tokio::test]
async fn test_service_cross_page_ownership_and_update_rollback() {
    let harness = TestHarness::new().await;

    let auth_admin = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };

    let block_service =
        spa_sax_backend::application::services::content_block_service::ContentBlockService::new(
            &harness.pool,
        );

    // 1. Create foreign page and foreign SPA section
    let other_page_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO pages (id, slug, title) VALUES ($1, $2, 'Foreign Page')")
        .bind(other_page_id)
        .bind(format!("page_{}", other_page_id.simple()))
        .execute(&harness.pool)
        .await
        .expect("Failed to insert foreign page");

    let foreign_spa_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order) VALUES ($1, $2, 'foreign-sec', 'Foreign', 'Foreign', 10)"
    )
    .bind(foreign_spa_id)
    .bind(other_page_id)
    .execute(&harness.pool)
    .await
    .expect("Failed to insert foreign spa section");

    let (home_sec_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM spa_sections WHERE page_id = (SELECT id FROM pages WHERE slug = 'home') AND section_key = 'about-us'",
    )
    .fetch_one(&harness.pool)
    .await
    .expect("Failed to fetch home about-us section");

    // 2. Attempt creating a ContentBlock referencing foreign_spa_id must return ValidationError
    let create_req = spa_sax_backend::application::dto::CreateContentBlockRequest {
        spa_section_id: foreign_spa_id,
        block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
        title: Some("Cross Page Block".to_string()),
        text: "Cross page attempt text".to_string(),
        media_id: None,
        is_visible: Some(true),
    };

    let create_res = block_service.create_block(&auth_admin, create_req).await;
    assert!(
        create_res.is_err(),
        "Service MUST reject cross-page spa_section_id during creation"
    );

    // 3. Create a valid ContentBlock on home/about-us
    let valid_create_req = spa_sax_backend::application::dto::CreateContentBlockRequest {
        spa_section_id: home_sec_id,
        block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
        title: Some("Valid Home Block".to_string()),
        text: "Valid home text".to_string(),
        media_id: None,
        is_visible: Some(true),
    };

    let valid_block = block_service
        .create_block(&auth_admin, valid_create_req)
        .await
        .expect("Valid block creation failed");

    // 4. Attempt updating valid_block to move it to foreign_spa_id -> MUST fail and rollback
    let update_req = spa_sax_backend::application::dto::UpdateContentBlockRequest {
        spa_section_id: Some(foreign_spa_id),
        block_type: None,
        title: Some("Updated Title Attempt".to_string()),
        text: Some("Updated text attempt".to_string()),
        media_id: None,
        is_visible: None,
    };

    let update_res = block_service
        .update_block(&auth_admin, valid_block.id, update_req)
        .await;

    assert!(
        update_res.is_err(),
        "Service MUST reject moving home block to foreign SPA section"
    );

    // 5. Verify transactional rollback: block in DB retains original title and original spa_section_id
    let re_fetched = block_service
        .get_block(&auth_admin, valid_block.id)
        .await
        .expect("Failed to re-fetch block after failed update");

    assert_eq!(re_fetched.spa_section_id, valid_block.spa_section_id);
    assert_eq!(re_fetched.title, Some("Valid Home Block".to_string()));
    assert_eq!(re_fetched.text, "Valid home text");

    // Clean up test fixtures
    common::cleanup_test_page(&harness.pool, other_page_id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, valid_block.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_deterministic_content_block_ordering() {
    let harness = TestHarness::new().await;

    let auth_admin = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };

    let block_service =
        spa_sax_backend::application::services::content_block_service::ContentBlockService::new(
            &harness.pool,
        );

    let (home_sec_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM spa_sections WHERE page_id = (SELECT id FROM pages WHERE slug = 'home') AND section_key = 'about-us'",
    )
    .fetch_one(&harness.pool)
    .await
    .expect("Failed to fetch home about-us section");

    // Manually set sort_order in DB if testing identical sort_order
    let block1 = block_service
        .create_block(
            &auth_admin,
            spa_sax_backend::application::dto::CreateContentBlockRequest {
                spa_section_id: home_sec_id,
                block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
                title: Some("Block 1".to_string()),
                text: "Text 1".to_string(),
                media_id: None,
                is_visible: Some(true),
            },
        )
        .await
        .unwrap();

    let block2 = block_service
        .create_block(
            &auth_admin,
            spa_sax_backend::application::dto::CreateContentBlockRequest {
                spa_section_id: home_sec_id,
                block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
                title: Some("Block 2".to_string()),
                text: "Text 2".to_string(),
                media_id: None,
                is_visible: Some(true),
            },
        )
        .await
        .unwrap();

    let block3 = block_service
        .create_block(
            &auth_admin,
            spa_sax_backend::application::dto::CreateContentBlockRequest {
                spa_section_id: home_sec_id,
                block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
                title: Some("Block 3".to_string()),
                text: "Text 3".to_string(),
                media_id: None,
                is_visible: Some(true),
            },
        )
        .await
        .unwrap();

    // Set identical sort_order for deterministic ID sorting check
    sqlx::query("UPDATE sections SET sort_order = 10 WHERE id IN ($1, $2, $3)")
        .bind(block1.id)
        .bind(block2.id)
        .bind(block3.id)
        .execute(&harness.pool)
        .await
        .unwrap();

    let blocks = block_service.list_blocks(&auth_admin, None).await.unwrap();
    let created_ids: Vec<uuid::Uuid> = blocks
        .into_iter()
        .filter(|b| b.id == block1.id || b.id == block2.id || b.id == block3.id)
        .map(|b| b.id)
        .collect();

    let mut expected_ids = vec![block1.id, block2.id, block3.id];
    expected_ids.sort(); // Sorts by UUID (id ASC)

    assert_eq!(
        created_ids, expected_ids,
        "ContentBlock list MUST be deterministically sorted by id ASC when sort_order is identical"
    );

    // Clean up test fixtures
    common::cleanup_content_block(&harness.pool, block1.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, block2.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, block3.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_admin_spa_sections_full_lifecycle() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    common::reset_home_sections_to_bootstrap(&harness.pool).await;

    // 1. Create section "Partners" & verify
    let req1 = serde_json::json!({ "title": "Partners" });
    let res1 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&req1)
        .dispatch()
        .await;

    assert_eq!(
        res1.status(),
        Status::Created,
        "Step 1: create partners status failed"
    );
    let sec1: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res1.into_json().await.unwrap();
    assert_eq!(sec1.data.title, "Partners", "Step 1: title match failed");
    assert_eq!(
        sec1.data.navigation_label, "Partners",
        "Step 1: nav label match failed"
    );
    assert_eq!(sec1.data.key, "partners", "Step 1: key match failed");
    assert!(sec1.data.is_visible, "Step 1: is_visible failed");

    // Immediate cleanup sec1
    common::cleanup_spa_section(&harness.pool, sec1.data.id)
        .await
        .unwrap();

    // 2. Custom navigation label "Our Strategic Partners" -> "Partners"
    let req2 = serde_json::json!({
        "title": "Our Strategic Partners",
        "navigation_label": "Partners"
    });
    let res2 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&req2)
        .dispatch()
        .await;

    assert_eq!(
        res2.status(),
        Status::Created,
        "Step 2: custom nav label create status failed"
    );
    let sec2: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res2.into_json().await.unwrap();
    assert_eq!(
        sec2.data.title, "Our Strategic Partners",
        "Step 2: title match failed"
    );
    assert_eq!(
        sec2.data.navigation_label, "Partners",
        "Step 2: nav label match failed"
    );
    assert_eq!(
        sec2.data.key, "our-strategic-partners",
        "Step 2: key match failed"
    );

    // Immediate cleanup sec2
    common::cleanup_spa_section(&harness.pool, sec2.data.id)
        .await
        .unwrap();

    // 3. Key collision allocation: Create two sections with title "Partners"
    let c1 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&req1)
        .dispatch()
        .await;
    let sec_col1: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        c1.into_json().await.unwrap();
    assert_eq!(
        sec_col1.data.key, "partners",
        "Step 3: first collision key failed"
    );

    let c2 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&req1)
        .dispatch()
        .await;
    let sec_col2: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        c2.into_json().await.unwrap();
    assert_eq!(
        sec_col2.data.key, "partners-2",
        "Step 3: second collision key failed"
    );

    // Immediate cleanup col1 & col2
    common::cleanup_spa_section(&harness.pool, sec_col1.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec_col2.data.id)
        .await
        .unwrap();

    // 4. Key Immutability on PATCH title
    let c3 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "title": "Press Room" }))
        .dispatch()
        .await;
    let sec_imm: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        c3.into_json().await.unwrap();
    assert_eq!(
        sec_imm.data.key, "press-room",
        "Step 4: create press-room key failed"
    );

    let patch_res = harness
        .client
        .patch(format!("/api/v1/admin/spa-sections/{}", sec_imm.data.id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "title": "Global Press Room" }))
        .dispatch()
        .await;
    assert_eq!(
        patch_res.status(),
        Status::Ok,
        "Step 4: patch status failed"
    );
    let patch_body: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        patch_res.into_json().await.unwrap();
    assert_eq!(
        patch_body.data.title, "Global Press Room",
        "Step 4: patched title match failed"
    );
    assert_eq!(
        patch_body.data.key, "press-room",
        "Step 4: Key MUST remain immutable on PATCH"
    );

    common::cleanup_spa_section(&harness.pool, sec_imm.data.id)
        .await
        .unwrap();

    // 5. Validation error: empty title
    let bad_res = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "title": "   " }))
        .dispatch()
        .await;
    assert_eq!(
        bad_res.status(),
        Status::UnprocessableEntity,
        "Step 5: empty title validation status failed"
    );

    // 6. Delete conflict test: attach ContentBlock to a section & attempt DELETE
    let c4 = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "title": "Temporary Awards" }))
        .dispatch()
        .await;
    let sec_del: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        c4.into_json().await.unwrap();

    let auth_admin = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };
    let block_service =
        spa_sax_backend::application::services::content_block_service::ContentBlockService::new(
            &harness.pool,
        );
    let created_block = block_service
        .create_block(
            &auth_admin,
            spa_sax_backend::application::dto::CreateContentBlockRequest {
                spa_section_id: sec_del.data.id,
                block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
                title: Some("Award Block".to_string()),
                text: "Award content".to_string(),
                media_id: None,
                is_visible: Some(true),
            },
        )
        .await
        .unwrap();

    // Attempt DELETE -> Conflict 409
    let del_conflict_res = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", sec_del.data.id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(
        del_conflict_res.status(),
        Status::Conflict,
        "Step 6: delete non-empty conflict status failed"
    );

    // Remove block & attempt DELETE -> OK 200
    common::cleanup_content_block(&harness.pool, created_block.id)
        .await
        .unwrap();
    let del_ok_res = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", sec_del.data.id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(
        del_ok_res.status(),
        Status::Ok,
        "Step 6: delete empty section status failed"
    );

    // GET by ID returns 404
    let get_404_res = harness
        .client
        .get(format!("/api/v1/admin/spa-sections/{}", sec_del.data.id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(
        get_404_res.status(),
        Status::NotFound,
        "Step 6: GET deleted section status failed"
    );

    common::cleanup_spa_section(&harness.pool, sec_del.data.id)
        .await
        .unwrap();

    common::reset_home_sections_to_bootstrap(&harness.pool).await;

    // 7. Dynamic reorder test on canonical 5 sections
    let list_res = harness
        .client
        .get("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;
    let list_body: SingleResponse<Vec<spa_sax_backend::application::dto::AdminSpaSectionDto>> =
        list_res.into_json().await.unwrap();
    let initial_items = list_body.data;
    assert_eq!(initial_items.len(), 5, "Step 7: initial items count failed");

    let mut reorder_payload = Vec::new();
    for (idx, item) in initial_items.iter().enumerate() {
        reorder_payload.push(serde_json::json!({
            "id": item.id,
            "sort_order": (idx as i32 + 1) * 10
        }));
    }

    let reorder_res = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": reorder_payload }))
        .dispatch()
        .await;
    assert_eq!(
        reorder_res.status(),
        Status::Ok,
        "Step 7: reorder status failed"
    );

    // Reorder failure case: negative sort order
    let mut bad_payload = reorder_payload.clone();
    bad_payload[0]["sort_order"] = serde_json::json!(-5);
    let bad_reorder_res = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": bad_payload }))
        .dispatch()
        .await;
    assert_eq!(
        bad_reorder_res.status(),
        Status::UnprocessableEntity,
        "Step 7: bad reorder status failed"
    );

    // Reset sort_orders back to original
    for item in &initial_items {
        sqlx::query("UPDATE spa_sections SET sort_order = $1 WHERE id = $2")
            .bind(item.sort_order)
            .bind(item.id)
            .execute(&harness.pool)
            .await
            .unwrap();
    }

    // 8. Permission check: invalid/unauthenticated token request MUST fail (401)
    let unauth_res = harness
        .client
        .get("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", "Bearer invalid_token_12345"))
        .dispatch()
        .await;

    assert_eq!(unauth_res.status().code, 401);
}

#[tokio::test]
async fn test_post_suite_home_bootstrap_verification() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    common::reset_home_sections_to_bootstrap(&harness.pool).await;

    // Query home page SPA sections
    let page: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
        .fetch_one(&harness.pool)
        .await
        .expect("Home page missing");

    let repo = spa_sax_backend::infrastructure::repositories::spa_section_repository::SpaSectionRepository::new(&harness.pool);
    let spa_sections = repo
        .list_for_page(page.0)
        .await
        .expect("Failed to list home SPA sections");

    assert_eq!(
        spa_sections.len(),
        5,
        "Home page must contain exactly 5 SPA sections"
    );

    let keys: Vec<&str> = spa_sections
        .iter()
        .map(|s| s.section_key.as_str())
        .collect();
    assert_eq!(
        keys,
        vec![
            "about-us",
            "our-works",
            "festivals",
            "gallery",
            "contact-us"
        ]
    );

    let orders: Vec<i32> = spa_sections.iter().map(|s| s.sort_order).collect();
    assert_eq!(orders, vec![10, 20, 30, 40, 50]);
}

#[tokio::test]
async fn test_concurrent_spa_section_creation() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let req_body = serde_json::json!({ "title": "Concurrent Partners" });

    let (res1, res2) = tokio::join!(
        harness
            .client
            .post("/api/v1/admin/spa-sections")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", harness.super_admin_token),
            ))
            .json(&req_body)
            .dispatch(),
        harness
            .client
            .post("/api/v1/admin/spa-sections")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", harness.super_admin_token),
            ))
            .json(&req_body)
            .dispatch()
    );

    assert_eq!(res1.status(), Status::Created);
    assert_eq!(res2.status(), Status::Created);

    let sec1: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res1.into_json().await.unwrap();
    let sec2: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res2.into_json().await.unwrap();

    let mut keys = vec![sec1.data.key.clone(), sec2.data.key.clone()];
    keys.sort();
    assert_eq!(keys, vec!["concurrent-partners", "concurrent-partners-2"]);

    assert_ne!(
        sec1.data.sort_order, sec2.data.sort_order,
        "Concurrent sections must have distinct sort orders"
    );

    // Clean up test fixtures
    common::cleanup_spa_section(&harness.pool, sec1.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec2.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_concurrent_delete_vs_content_block_create() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // 1. Create a section
    let res = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "title": "Competing Section" }))
        .dispatch()
        .await;

    assert_eq!(res.status(), Status::Created);
    let sec: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res.into_json().await.unwrap();
    let sec_id = sec.data.id;

    // 2. Competing delete vs block creation
    let auth_user = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };

    let block_svc =
        spa_sax_backend::application::services::content_block_service::ContentBlockService::new(
            &harness.pool,
        );
    let sec_svc =
        spa_sax_backend::application::services::spa_section_service::SpaSectionService::new(
            &harness.pool,
        );

    let (del_res, block_res) = tokio::join!(
        sec_svc.delete_section(&auth_user, sec_id),
        block_svc.create_block(
            &auth_user,
            spa_sax_backend::application::dto::CreateContentBlockRequest {
                block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
                title: Some("Compete Block".to_string()),
                text: "Competing block text".to_string(),
                is_visible: Some(true),
                spa_section_id: sec_id,
                media_id: None,
            }
        )
    );

    // 3. Assert forbidden state (deleted section AND active content block) never occurs
    let is_deleted: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM spa_sections WHERE id = $1 AND deleted_at IS NOT NULL)",
    )
    .bind(sec_id)
    .fetch_one(&harness.pool)
    .await
    .unwrap();

    let block_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sections WHERE spa_section_id = $1 AND deleted_at IS NULL",
    )
    .bind(sec_id)
    .fetch_one(&harness.pool)
    .await
    .unwrap();

    if is_deleted.0 {
        assert_eq!(
            block_count.0, 0,
            "Forbidden state: section is deleted but active content block exists!"
        );
        assert!(block_res.is_err());
    } else {
        assert!(del_res.is_err());
        if let Ok(block) = block_res {
            common::cleanup_content_block(&harness.pool, block.id)
                .await
                .unwrap();
        }
    }

    common::cleanup_spa_section(&harness.pool, sec_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_admin_spa_sections_reorder_real_swap_and_failure_matrix() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let repo = spa_sax_backend::infrastructure::repositories::spa_section_repository::SpaSectionRepository::new(&harness.pool);
    let home_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
        .fetch_one(&harness.pool)
        .await
        .unwrap();

    let active_before = repo.list_admin_for_page(home_id.0).await.unwrap();
    assert!(active_before.len() >= 5);

    // 1. Perform REAL swap of order for first two sections
    let mut swapped_payload = Vec::new();
    for (idx, item) in active_before.iter().enumerate() {
        let target_order = if idx == 0 {
            active_before[1].sort_order
        } else if idx == 1 {
            active_before[0].sort_order
        } else {
            item.sort_order
        };
        swapped_payload.push(serde_json::json!({
            "id": item.id,
            "sort_order": target_order
        }));
    }

    let swap_res = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": swapped_payload }))
        .dispatch()
        .await;

    assert_eq!(swap_res.status(), Status::Ok);

    let active_after_swap = repo.list_admin_for_page(home_id.0).await.unwrap();
    assert_eq!(active_after_swap[0].id, active_before[1].id);
    assert_eq!(active_after_swap[1].id, active_before[0].id);

    // 2. Failure Matrix Tests
    // Duplicate ID
    let mut dup_id_payload = swapped_payload.clone();
    dup_id_payload[1]["id"] = dup_id_payload[0]["id"].clone();
    let res_dup_id = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": dup_id_payload }))
        .dispatch()
        .await;
    assert_eq!(res_dup_id.status(), Status::UnprocessableEntity);

    // Missing active section
    let missing_section_payload = swapped_payload[..swapped_payload.len() - 1].to_vec();
    let res_missing = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": missing_section_payload }))
        .dispatch()
        .await;
    assert_eq!(res_missing.status(), Status::UnprocessableEntity);

    // Unknown UUID
    let mut unknown_uuid_payload = swapped_payload.clone();
    unknown_uuid_payload[0]["id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
    let res_unknown = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": unknown_uuid_payload }))
        .dispatch()
        .await;
    assert_eq!(res_unknown.status(), Status::UnprocessableEntity);

    // Negative sort_order
    let mut neg_order_payload = swapped_payload.clone();
    neg_order_payload[0]["sort_order"] = serde_json::json!(-10);
    let res_neg = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": neg_order_payload }))
        .dispatch()
        .await;
    assert_eq!(res_neg.status(), Status::UnprocessableEntity);

    // Duplicate sort_order
    let mut dup_order_payload = swapped_payload.clone();
    dup_order_payload[1]["sort_order"] = dup_order_payload[0]["sort_order"].clone();
    let res_dup_order = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&serde_json::json!({ "items": dup_order_payload }))
        .dispatch()
        .await;
    assert_eq!(res_dup_order.status(), Status::UnprocessableEntity);

    // 3. Restore snapshot order
    for item in &active_before {
        sqlx::query("UPDATE spa_sections SET sort_order = $1 WHERE id = $2")
            .bind(item.sort_order)
            .bind(item.id)
            .execute(&harness.pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_regular_admin_spa_sections_permissions() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // Create a regular Admin user (Role::Admin) directly in DB
    let hash = PasswordService::hash_password("AdminPass123!").unwrap();
    let admin_id = uuid::Uuid::new_v4();
    let admin_email = format!("admin_{}@example.com", admin_id.simple());

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, role, is_active) VALUES ($1, $2, $3, 'Admin User', 'admin', TRUE)",
    )
    .bind(admin_id)
    .bind(&admin_email)
    .bind(&hash)
    .execute(&harness.pool)
    .await
    .unwrap();

    let token = spa_sax_backend::infrastructure::auth::TokenService::generate_access_token(
        admin_id,
        &admin_email,
        Role::Admin,
        &harness.config.jwt_access_secret,
        900,
    )
    .unwrap();

    // 1. GET list -> OK 200
    let res_list = harness
        .client
        .get("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_list.status(), Status::Ok);

    // 2. POST create -> OK 201
    let res_create = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Admin Created" }))
        .dispatch()
        .await;
    assert_eq!(res_create.status(), Status::Created);
    let created: SingleResponse<spa_sax_backend::application::dto::AdminSpaSectionDto> =
        res_create.into_json().await.unwrap();

    // 3. GET one -> OK 200
    let res_get = harness
        .client
        .get(format!("/api/v1/admin/spa-sections/{}", created.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_get.status(), Status::Ok);

    // 4. PATCH -> OK 200
    let res_patch = harness
        .client
        .patch(format!("/api/v1/admin/spa-sections/{}", created.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Admin Updated" }))
        .dispatch()
        .await;
    assert_eq!(res_patch.status(), Status::Ok);

    // 5. DELETE -> OK 200
    let res_del = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", created.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_del.status(), Status::Ok);

    // Deactivate admin user
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(admin_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    // Inactive admin GET list -> 403 Forbidden
    let res_inact = harness
        .client
        .get("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_inact.status(), Status::Forbidden);

    common::cleanup_spa_section(&harness.pool, created.data.id)
        .await
        .unwrap();
    common::cleanup_test_user(&harness.pool, admin_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_foreign_page_spa_section_get_404() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // Create foreign page & section
    let foreign_page_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pages (id, slug, title) VALUES ($1, 'foreign-page-test', 'Foreign Page')",
    )
    .bind(foreign_page_id)
    .execute(&harness.pool)
    .await
    .unwrap();

    let foreign_sec_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order) VALUES ($1, $2, 'foreign-sec', 'Foreign Sec', 'Foreign Sec', 10)",
    )
    .bind(foreign_sec_id)
    .bind(foreign_page_id)
    .execute(&harness.pool)
    .await
    .unwrap();

    // GET /api/v1/admin/spa-sections/{foreign_sec_id} -> 404
    let res = harness
        .client
        .get(format!("/api/v1/admin/spa-sections/{}", foreign_sec_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(res.status(), Status::NotFound);

    sqlx::query("DELETE FROM spa_sections WHERE id = $1")
        .bind(foreign_sec_id)
        .execute(&harness.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(foreign_page_id)
        .execute(&harness.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_spa_section_validation_coverage() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let cases = vec![
        (serde_json::json!({ "title": "" }), "empty title"),
        (serde_json::json!({ "title": "   " }), "whitespace title"),
        (
            serde_json::json!({ "title": "a".repeat(256) }),
            "title > 255 chars",
        ),
        (
            serde_json::json!({ "title": "Valid Title", "navigation_label": "" }),
            "empty nav label",
        ),
        (
            serde_json::json!({ "title": "Valid Title", "navigation_label": "   " }),
            "whitespace nav label",
        ),
        (
            serde_json::json!({ "title": "Valid Title", "navigation_label": "a".repeat(101) }),
            "nav label > 100 chars",
        ),
    ];

    for (payload, desc) in cases {
        let res = harness
            .client
            .post("/api/v1/admin/spa-sections")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", harness.super_admin_token),
            ))
            .json(&payload)
            .dispatch()
            .await;
        assert_eq!(
            res.status(),
            Status::UnprocessableEntity,
            "Validation test failed for case: {}",
            desc
        );
    }
}

#[tokio::test]
async fn test_content_block_explicit_section_and_validation() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Create dynamic section "Partners"
    let res_sec = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Partners" }))
        .dispatch()
        .await;
    assert_eq!(res_sec.status(), Status::Created);
    let partners_sec: SingleResponse<AdminSpaSectionDto> = res_sec.into_json().await.unwrap();

    // 2. Create block with explicit spa_section_id -> 201 Created
    let res_block = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": partners_sec.data.id,
            "block_type": "text",
            "title": "Partner Info",
            "text": "We partner with leading organizations."
        }))
        .dispatch()
        .await;
    assert_eq!(res_block.status(), Status::Created);
    let block_dto: SingleResponse<AdminContentBlockDto> = res_block.into_json().await.unwrap();
    assert_eq!(block_dto.data.spa_section_id, partners_sec.data.id);
    assert_eq!(block_dto.data.section_key, "partners");
    assert_eq!(block_dto.data.section_title, "Partners");

    // 3. Create block WITHOUT spa_section_id -> 422 Unprocessable Entity
    let res_no_sec = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "block_type": "text",
            "title": "No Section Block",
            "text": "Text without section"
        }))
        .dispatch()
        .await;
    assert_eq!(res_no_sec.status(), Status::UnprocessableEntity);

    // Verify nothing was silently added to about-us
    let about_blocks: SingleResponse<Vec<AdminContentBlockDto>> = harness
        .client
        .get("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert!(about_blocks
        .data
        .iter()
        .all(|b| b.title.as_deref() != Some("No Section Block")));

    // 4. Create block in HIDDEN section -> 201 Created
    let res_hidden_sec = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Hidden Section", "is_visible": false }))
        .dispatch()
        .await;
    let hidden_sec: SingleResponse<AdminSpaSectionDto> = res_hidden_sec.into_json().await.unwrap();

    let res_hidden_block = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": hidden_sec.data.id,
            "block_type": "text",
            "title": "Hidden Block",
            "text": "Content inside hidden section"
        }))
        .dispatch()
        .await;
    assert_eq!(res_hidden_block.status(), Status::Created);

    // 5. Create block in DELETED section -> 404 Not Found
    let res_empty = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Temporary Empty" }))
        .dispatch()
        .await;
    let empty_sec: SingleResponse<AdminSpaSectionDto> = res_empty.into_json().await.unwrap();

    harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", empty_sec.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;

    let res_del_create = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": empty_sec.data.id,
            "block_type": "text",
            "title": "Invalid Target Block",
            "text": "Attempt text"
        }))
        .dispatch()
        .await;
    assert_eq!(res_del_create.status(), Status::NotFound);

    // 6. Create block in FOREIGN page section -> 404 Not Found
    let other_page_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO pages (id, slug, title) VALUES ($1, $2, 'Foreign Page')")
        .bind(other_page_id)
        .bind(format!("page_{}", other_page_id.simple()))
        .execute(&harness.pool)
        .await
        .unwrap();

    let foreign_sec_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order) VALUES ($1, $2, 'foreign-key', 'Foreign', 'Foreign', 10)"
    )
    .bind(foreign_sec_id)
    .bind(other_page_id)
    .execute(&harness.pool)
    .await
    .unwrap();

    let res_foreign = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": foreign_sec_id,
            "block_type": "text",
            "title": "Foreign Target",
            "text": "Attempt text"
        }))
        .dispatch()
        .await;
    assert_eq!(res_foreign.status(), Status::NotFound);

    // Cleanup
    common::cleanup_test_page(&harness.pool, other_page_id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, block_dto.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, partners_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, hidden_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, empty_sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_content_block_list_filtering_and_ordering() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Create sections Partners & Awards
    let p_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Partners List Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let a_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Awards List Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Create blocks in Partners (A, B) and Awards (C)
    let block_a: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": p_sec.data.id,
            "block_type": "text",
            "title": "Block A",
            "text": "Content A"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let block_b: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": p_sec.data.id,
            "block_type": "text",
            "title": "Block B",
            "text": "Content B"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let block_c: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": a_sec.data.id,
            "block_type": "text",
            "title": "Block C",
            "text": "Content C"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Filter GET by Partners -> returns ONLY [A, B]
    let res_filter_p = harness
        .client
        .get(format!(
            "/api/v1/admin/content-blocks?spa_section_id={}",
            p_sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_filter_p.status(), Status::Ok);
    let p_blocks: SingleResponse<Vec<AdminContentBlockDto>> =
        res_filter_p.into_json().await.unwrap();
    assert_eq!(p_blocks.data.len(), 2);
    assert_eq!(p_blocks.data[0].id, block_a.data.id);
    assert_eq!(p_blocks.data[1].id, block_b.data.id);
    assert_eq!(p_blocks.data[0].section_key, p_sec.data.key);
    assert_eq!(p_blocks.data[1].section_key, p_sec.data.key);

    // Filter GET by empty section -> returns 200 []
    let empty_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Empty Filter Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let res_filter_empty = harness
        .client
        .get(format!(
            "/api/v1/admin/content-blocks?spa_section_id={}",
            empty_sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_filter_empty.status(), Status::Ok);
    let empty_blocks: SingleResponse<Vec<AdminContentBlockDto>> =
        res_filter_empty.into_json().await.unwrap();
    assert!(empty_blocks.data.is_empty());

    // Filter GET by unknown UUID -> 404 Not Found
    let res_filter_unknown = harness
        .client
        .get(format!(
            "/api/v1/admin/content-blocks?spa_section_id={}",
            uuid::Uuid::new_v4()
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_filter_unknown.status(), Status::NotFound);

    // Unfiltered GET returns all blocks ordered by SpaSection.sort_order ASC, ContentBlock.sort_order ASC, ContentBlock.id ASC
    let res_unfiltered = harness
        .client
        .get("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_unfiltered.status(), Status::Ok);
    let all_blocks: SingleResponse<Vec<AdminContentBlockDto>> =
        res_unfiltered.into_json().await.unwrap();
    assert!(all_blocks.data.iter().any(|b| b.id == block_a.data.id));
    assert!(all_blocks.data.iter().any(|b| b.id == block_c.data.id));

    // Cleanup
    common::cleanup_content_block(&harness.pool, block_a.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, block_b.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, block_c.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, p_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, a_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, empty_sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_content_block_move_between_sections() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    let sec1: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Source Section" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let sec2: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Target Section" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let block_x: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec1.data.id,
            "block_type": "text",
            "title": "Movable Block",
            "text": "Text to move"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 1. Move block X from sec1 to sec2
    let res_move = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", block_x.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "spa_section_id": sec2.data.id }))
        .dispatch()
        .await;
    assert_eq!(res_move.status(), Status::Ok);
    let moved_dto: SingleResponse<AdminContentBlockDto> = res_move.into_json().await.unwrap();
    assert_eq!(moved_dto.data.spa_section_id, sec2.data.id);
    assert_eq!(moved_dto.data.section_key, sec2.data.key);

    // 2. Move to invalid/unknown section -> 404 Not Found & rollback
    let res_invalid_move = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", block_x.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "spa_section_id": uuid::Uuid::new_v4() }))
        .dispatch()
        .await;
    assert_eq!(res_invalid_move.status(), Status::NotFound);

    // Verify block retained sec2
    let re_fetch: SingleResponse<AdminContentBlockDto> = harness
        .client
        .get(format!("/api/v1/admin/content-blocks/{}", block_x.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(re_fetch.data.spa_section_id, sec2.data.id);

    // Cleanup
    common::cleanup_content_block(&harness.pool, block_x.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec1.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec2.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_concurrent_content_block_creation_same_section() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let auth_user = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };

    let sec_svc =
        spa_sax_backend::application::services::spa_section_service::SpaSectionService::new(
            &harness.pool,
        );
    let block_svc =
        spa_sax_backend::application::services::content_block_service::ContentBlockService::new(
            &harness.pool,
        );

    let sec_dto = sec_svc
        .create_section(
            &auth_user,
            spa_sax_backend::application::dto::CreateSpaSectionRequest {
                title: "Concurrent Blocks Section".to_string(),
                navigation_label: None,
            },
        )
        .await
        .unwrap();

    let req1 = spa_sax_backend::application::dto::CreateContentBlockRequest {
        spa_section_id: sec_dto.id,
        block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
        title: Some("Block 1".to_string()),
        text: "Text 1".to_string(),
        media_id: None,
        is_visible: Some(true),
    };

    let req2 = spa_sax_backend::application::dto::CreateContentBlockRequest {
        spa_section_id: sec_dto.id,
        block_type: spa_sax_backend::domain::sections::ContentBlockType::Text,
        title: Some("Block 2".to_string()),
        text: "Text 2".to_string(),
        media_id: None,
        is_visible: Some(true),
    };

    let (res1, res2) = tokio::join!(
        block_svc.create_block(&auth_user, req1),
        block_svc.create_block(&auth_user, req2)
    );

    let b1 = res1.expect("Concurrent block 1 creation failed");
    let b2 = res2.expect("Concurrent block 2 creation failed");

    assert_ne!(b1.id, b2.id);
    assert_ne!(b1.sort_order, b2.sort_order);

    common::cleanup_content_block(&harness.pool, b1.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, b2.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec_dto.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_per_section_content_block_reorder_full_matrix() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    let sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Reorder Test Section" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b1: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec.data.id,
            "block_type": "text",
            "title": "Block A",
            "text": "Text A"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b2: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec.data.id,
            "block_type": "text",
            "title": "Block B",
            "text": "Text B"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b3: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec.data.id,
            "block_type": "text",
            "title": "Block C",
            "text": "Text C"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 1. Valid reorder: swap [b3, b1, b2]
    let res_reorder = harness
        .client
        .post(format!(
            "/api/v1/admin/spa-sections/{}/content-blocks/reorder",
            sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "items": [
                { "id": b3.data.id, "sort_order": 10 },
                { "id": b1.data.id, "sort_order": 20 },
                { "id": b2.data.id, "sort_order": 30 }
            ]
        }))
        .dispatch()
        .await;
    assert_eq!(res_reorder.status(), Status::Ok);

    // Verify ordering via list filter GET
    let list_res: SingleResponse<Vec<AdminContentBlockDto>> = harness
        .client
        .get(format!(
            "/api/v1/admin/content-blocks?spa_section_id={}",
            sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    assert_eq!(list_res.data.len(), 3);
    assert_eq!(list_res.data[0].id, b3.data.id);
    assert_eq!(list_res.data[1].id, b1.data.id);
    assert_eq!(list_res.data[2].id, b2.data.id);

    // 2. Empty section reorder test -> 200 OK
    let empty_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Empty Reorder Section" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let res_empty_reorder = harness
        .client
        .post(format!(
            "/api/v1/admin/spa-sections/{}/content-blocks/reorder",
            empty_sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "items": [] }))
        .dispatch()
        .await;
    assert_eq!(res_empty_reorder.status(), Status::Ok);

    // Cleanup
    common::cleanup_content_block(&harness.pool, b1.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, b2.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, b3.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, empty_sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_section_delete_after_block_move() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    let src_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Source Section Delete Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let tgt_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Target Section Delete Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let block: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": src_sec.data.id,
            "block_type": "text",
            "title": "Movable Delete Block",
            "text": "Text"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 1. DELETE src_sec -> 409 Conflict because block exists inside it
    let res_del1 = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", src_sec.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_del1.status(), Status::Conflict);

    // 2. Move block to tgt_sec -> 200 OK
    let res_move = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", block.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "spa_section_id": tgt_sec.data.id }))
        .dispatch()
        .await;
    assert_eq!(res_move.status(), Status::Ok);

    // 3. DELETE src_sec -> 200 OK now that section is empty!
    let res_del2 = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", src_sec.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_del2.status(), Status::Ok);

    // Cleanup
    common::cleanup_content_block(&harness.pool, block.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, src_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, tgt_sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_openapi_representation_regression() {
    use utoipa::openapi::path::PathItemType;
    use utoipa::OpenApi;

    let openapi = spa_sax_backend::bootstrap::ApiDoc::openapi();

    let paths = &openapi.paths;

    let sec_path = paths
        .paths
        .get("/api/v1/admin/spa-sections")
        .expect("OpenAPI missing /api/v1/admin/spa-sections path");
    assert!(sec_path.operations.contains_key(&PathItemType::Get));
    assert!(sec_path.operations.contains_key(&PathItemType::Post));

    let sec_id_path = paths
        .paths
        .get("/api/v1/admin/spa-sections/{id}")
        .expect("OpenAPI missing /api/v1/admin/spa-sections/{id} path");
    assert!(sec_id_path.operations.contains_key(&PathItemType::Get));
    assert!(sec_id_path.operations.contains_key(&PathItemType::Patch));
    assert!(sec_id_path.operations.contains_key(&PathItemType::Delete));

    let cb_path = paths
        .paths
        .get("/api/v1/admin/content-blocks")
        .expect("OpenAPI missing /api/v1/admin/content-blocks path");
    assert!(cb_path.operations.contains_key(&PathItemType::Get));
    assert!(cb_path.operations.contains_key(&PathItemType::Post));

    let cb_id_path = paths
        .paths
        .get("/api/v1/admin/content-blocks/{id}")
        .expect("OpenAPI missing /api/v1/admin/content-blocks/{id} path");
    assert!(cb_id_path.operations.contains_key(&PathItemType::Get));
    assert!(cb_id_path.operations.contains_key(&PathItemType::Patch));
    assert!(cb_id_path.operations.contains_key(&PathItemType::Delete));

    let reorder_path = paths
        .paths
        .get("/api/v1/admin/spa-sections/{spa_section_id}/content-blocks/reorder")
        .expect(
            "OpenAPI missing /api/v1/admin/spa-sections/{spa_section_id}/content-blocks/reorder path",
        );
    assert!(reorder_path.operations.contains_key(&PathItemType::Post));

    let components = openapi.components.expect("OpenAPI missing components");
    let schemas = components.schemas;

    assert!(schemas.contains_key("AdminSpaSectionDto"));
    assert!(schemas.contains_key("CreateSpaSectionRequest"));
    assert!(schemas.contains_key("UpdateSpaSectionRequest"));
    assert!(schemas.contains_key("ReorderSpaSectionsRequest"));
    assert!(schemas.contains_key("ReorderSpaSectionItem"));
    assert!(schemas.contains_key("AdminContentBlockDto"));
    assert!(schemas.contains_key("CreateContentBlockRequest"));
    assert!(schemas.contains_key("UpdateContentBlockRequest"));
    assert!(schemas.contains_key("ReorderContentBlocksRequest"));
    assert!(schemas.contains_key("ReorderContentBlockItem"));
    assert!(schemas.contains_key("PublicPageResponse"));
    assert!(schemas.contains_key("PublicSpaSectionDto"));
}

#[tokio::test]
async fn test_foreign_content_block_admin_isolation() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Setup foreign page, foreign section, foreign content block directly in DB
    let foreign_page_id = uuid::Uuid::new_v4();
    let foreign_slug = format!("foreign-page-{}", foreign_page_id);
    sqlx::query("INSERT INTO pages (id, title, slug) VALUES ($1, 'Foreign Page', $2)")
        .bind(foreign_page_id)
        .bind(&foreign_slug)
        .execute(&harness.pool)
        .await
        .unwrap();

    let foreign_sec_id = uuid::Uuid::new_v4();
    let foreign_sec_key = format!("foreign-sec-{}", foreign_sec_id);
    sqlx::query("INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible) VALUES ($1, $2, $3, 'Foreign Sec', 'Foreign Sec', 10, true)")
        .bind(foreign_sec_id)
        .bind(foreign_page_id)
        .bind(&foreign_sec_key)
        .execute(&harness.pool)
        .await
        .unwrap();

    let foreign_block_id = uuid::Uuid::new_v4();
    let foreign_block_key = format!("foreign_block_{}", foreign_block_id.simple());
    let initial_content = serde_json::json!({ "text": "Foreign Initial Text" });
    sqlx::query("INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content, sort_order, is_visible, status) VALUES ($1, $2, $3, $4, 'text', 'Foreign Title', $5, 10, true, 'published')")
        .bind(foreign_block_id)
        .bind(foreign_page_id)
        .bind(foreign_sec_id)
        .bind(&foreign_block_key)
        .bind(&initial_content)
        .execute(&harness.pool)
        .await
        .unwrap();

    // 2. GET foreign ContentBlock -> 404
    let res_get = harness
        .client
        .get(format!("/api/v1/admin/content-blocks/{}", foreign_block_id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_get.status(), Status::NotFound);

    // 3. PATCH foreign ContentBlock -> 404 & verify DB unchanged
    let res_patch = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", foreign_block_id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Must Not Change" }))
        .dispatch()
        .await;
    assert_eq!(res_patch.status(), Status::NotFound);

    let db_row: (String, Option<String>) =
        sqlx::query_as("SELECT section_type, title FROM sections WHERE id = $1")
            .bind(foreign_block_id)
            .fetch_one(&harness.pool)
            .await
            .unwrap();
    assert_eq!(db_row.1.as_deref(), Some("Foreign Title"));

    // 4. DELETE foreign ContentBlock -> 404 & verify DB deleted_at IS NULL
    let res_del = harness
        .client
        .delete(format!("/api/v1/admin/content-blocks/{}", foreign_block_id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(res_del.status(), Status::NotFound);

    let deleted_at: (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT deleted_at FROM sections WHERE id = $1")
            .bind(foreign_block_id)
            .fetch_one(&harness.pool)
            .await
            .unwrap();
    assert!(deleted_at.0.is_none());

    // 5. Unfiltered GET /api/v1/admin/content-blocks must exclude foreign block
    let res_list: SingleResponse<Vec<AdminContentBlockDto>> = harness
        .client
        .get("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert!(!res_list.data.iter().any(|b| b.id == foreign_block_id));

    // Cleanup foreign test fixtures
    if let Err(e) = common::cleanup_test_page(&harness.pool, foreign_page_id).await {
        panic!("cleanup_test_page error: {:?}", e);
    }
}

#[tokio::test]
async fn test_media_backed_section_move_preserves_media_identity() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Create dynamic sections "Partners Media Test" & "Awards Media Test"
    let p_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Partners Media Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let a_sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Awards Media Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 2. Create media asset fixture directly in DB
    let media_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO media_assets (id, media_type, storage_provider, original_filename, stored_filename, mime_type, file_size, status) VALUES ($1, 'image', 'local', 'test.jpg', 'test.jpg', 'image/jpeg', 1024, 'active')"
    )
    .bind(media_id)
    .execute(&harness.pool)
    .await
    .unwrap();

    // 3. Create text_image ContentBlock in Partners section
    let block: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": p_sec.data.id,
            "block_type": "text_image",
            "title": "Partner Image Block",
            "text": "Partner Description",
            "media_id": media_id
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Query section_media relation identity before move
    let sm_before: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, media_asset_id FROM section_media WHERE section_id = $1")
            .bind(block.data.id)
            .fetch_one(&harness.pool)
            .await
            .unwrap();
    assert_eq!(sm_before.1, media_id);

    // 4. Move block to Awards section via PATCH with ONLY spa_section_id (NO media_id in payload)
    let res_move = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", block.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": a_sec.data.id
        }))
        .dispatch()
        .await;
    assert_eq!(res_move.status(), Status::Ok);
    let moved_dto: SingleResponse<AdminContentBlockDto> = res_move.into_json().await.unwrap();
    assert_eq!(moved_dto.data.spa_section_id, a_sec.data.id);
    assert!(moved_dto.data.media.is_some());
    assert_eq!(moved_dto.data.media.unwrap().id, media_id);

    // 5. Query section_media relation identity after move -> MUST BE UNCHANGED
    let sm_after: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, media_asset_id FROM section_media WHERE section_id = $1")
            .bind(block.data.id)
            .fetch_one(&harness.pool)
            .await
            .unwrap();
    assert_eq!(
        sm_after.0, sm_before.0,
        "section_media relation row ID MUST remain identical on section-only move"
    );
    assert_eq!(sm_after.1, media_id);

    // Cleanup
    common::cleanup_content_block(&harness.pool, block.data.id)
        .await
        .unwrap();
    sqlx::query("DELETE FROM media_assets WHERE id = $1")
        .bind(media_id)
        .execute(&harness.pool)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, p_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, a_sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_deleted_spa_section_reorder_returns_404() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Create dynamic section "Deleted Reorder Sec"
    let sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Deleted Reorder Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 2. Soft-delete the section
    let sec_svc =
        spa_sax_backend::application::services::spa_section_service::SpaSectionService::new(
            &harness.pool,
        );
    let auth_user = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin Test".to_string(),
        role: Role::SuperAdmin,
        is_active: true,
    };
    sec_svc
        .delete_section(&auth_user, sec.data.id)
        .await
        .unwrap();

    // 3. Attempt per-section reorder on deleted section -> 404 Not Found
    let res_reorder = harness
        .client
        .post(format!(
            "/api/v1/admin/spa-sections/{}/content-blocks/reorder",
            sec.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "items": [] }))
        .dispatch()
        .await;
    assert_eq!(res_reorder.status(), Status::NotFound);

    // Cleanup
    common::cleanup_spa_section(&harness.pool, sec.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_public_page_grouped_response_and_visibility_matrix() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Clean up dynamic sections so only bootstrap sections remain
    sqlx::query("DELETE FROM spa_sections WHERE page_id = (SELECT id FROM pages WHERE slug = 'home') AND section_key NOT IN ('about-us', 'our-works', 'festivals', 'gallery', 'contact-us')")
        .execute(&harness.pool)
        .await
        .ok();

    // 2. Query GET /api/v1/public/page (unauthenticated)
    let res = harness.client.get("/api/v1/public/page").dispatch().await;
    assert_eq!(res.status(), Status::Ok);
    let public_resp: SingleResponse<PublicPageResponse> = res.into_json().await.unwrap();

    assert_eq!(public_resp.data.page.slug, "home");
    assert!(public_resp.data.sections.len() >= 5);
    let bootstrap_keys: Vec<&str> = public_resp
        .data
        .sections
        .iter()
        .map(|s| s.key.as_str())
        .collect();
    assert!(bootstrap_keys.contains(&"about-us"));
    assert!(bootstrap_keys.contains(&"our-works"));
    assert!(bootstrap_keys.contains(&"festivals"));

    // 3. Create dynamic section "Public Visible Sec" (is_visible = true) & "Public Hidden Sec" (is_visible = false)
    let sec_vis: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Public Visible Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let sec_hid: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Public Hidden Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Hide sec_hid
    harness
        .client
        .patch(format!("/api/v1/admin/spa-sections/{}", sec_hid.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "is_visible": false }))
        .dispatch()
        .await;

    // Create visible block & hidden block inside sec_vis
    let b1: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_vis.data.id,
            "block_type": "text",
            "title": "Visible Block 1",
            "text": "Text 1",
            "is_visible": true
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b2: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_vis.data.id,
            "block_type": "text",
            "title": "Hidden Block 2",
            "text": "Text 2",
            "is_visible": false
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Create visible block inside sec_hid (should NOT leak because section is hidden)
    let b_hid_sec: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_hid.data.id,
            "block_type": "text",
            "title": "Block inside Hidden Sec",
            "text": "Text Hidden Sec",
            "is_visible": true
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Create empty visible section
    let sec_empty: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Empty Visible Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 4. Query public page again & verify visibility matrix
    let res2 = harness.client.get("/api/v1/public/page").dispatch().await;
    assert_eq!(res2.status(), Status::Ok);
    let page2: SingleResponse<PublicPageResponse> = res2.into_json().await.unwrap();

    // Verify hidden section is COMPLETELY ABSENT
    assert!(
        !page2.data.sections.iter().any(|s| s.id == sec_hid.data.id),
        "Hidden SpaSection MUST NOT appear in public response"
    );

    // Verify visible empty section IS RETURNED with blocks: []
    let empty_dto = page2
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_empty.data.id)
        .expect("Visible empty SpaSection MUST appear in public response");
    assert!(
        empty_dto.blocks.is_empty(),
        "Visible empty section must have blocks: []"
    );

    // Verify visible section contains ONLY visible block b1 (b2 is hidden)
    let vis_sec_dto = page2
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_vis.data.id)
        .expect("Visible SpaSection MUST appear");
    assert_eq!(vis_sec_dto.blocks.len(), 1);
    assert_eq!(vis_sec_dto.blocks[0].id, b1.data.id);
    assert_eq!(
        vis_sec_dto.blocks[0].title.as_deref(),
        Some("Visible Block 1")
    );

    // Cleanup test fixtures
    common::cleanup_content_block(&harness.pool, b1.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, b2.data.id)
        .await
        .unwrap();
    common::cleanup_content_block(&harness.pool, b_hid_sec.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec_vis.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec_hid.data.id)
        .await
        .unwrap();
    common::cleanup_spa_section(&harness.pool, sec_empty.data.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_normal_admin_media_upload_permissions() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // 1. Create a regular active admin user (role = admin)
    let admin_id = uuid::Uuid::new_v4();
    let admin_email = format!("normal_admin_{}@example.com", admin_id.simple());
    let pass_hash = PasswordService::hash_password("NormalAdmin123!").unwrap();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, display_name, role, is_active)
        VALUES ($1, $2, $3, 'Normal Admin Media Test', 'admin', TRUE)
        "#,
    )
    .bind(admin_id)
    .bind(&admin_email)
    .bind(&pass_hash)
    .execute(&harness.pool)
    .await
    .unwrap();

    let admin_token = spa_sax_backend::infrastructure::auth::TokenService::generate_access_token(
        admin_id,
        &admin_email,
        Role::Admin,
        "test_jwt_access_secret_key_min_32_bytes_123456",
        900,
    )
    .unwrap();

    // 2. Perform YouTube media creation as normal admin (role = admin) -> MUST SUCCEED (200 OK)
    let res = harness
        .client
        .post("/api/v1/admin/media/youtube")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .json(&serde_json::json!({
            "youtube_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "title": "Normal Admin YouTube Media"
        }))
        .dispatch()
        .await;

    assert_eq!(
        res.status(),
        Status::Ok,
        "Normal Admin (role = admin) MUST be allowed to create media"
    );
    let media_res: SingleResponse<spa_sax_backend::application::dto::AdminMediaDto> =
        res.into_json().await.unwrap();

    // Verify row exists in DB
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_assets WHERE id = $1")
        .bind(match media_res.data {
            spa_sax_backend::application::dto::AdminMediaDto::Youtube { id, .. } => id,
            _ => panic!("Expected Youtube media variant"),
        })
        .fetch_one(&harness.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);

    // 3. Deactivate normal admin user -> MUST BE REJECTED (401 / 403)
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(admin_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    let res_deactive = harness
        .client
        .post("/api/v1/admin/media/youtube")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .json(&serde_json::json!({
            "youtube_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        }))
        .dispatch()
        .await;

    assert!(
        res_deactive.status() == Status::Unauthorized || res_deactive.status() == Status::Forbidden,
        "Deactivated admin MUST be rejected when uploading/creating media"
    );

    // Cleanup
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(admin_id)
        .execute(&harness.pool)
        .await
        .ok();
}

#[tokio::test]
async fn test_public_page_malformed_media_resilience() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    let token = harness.super_admin_token.clone();

    // 1. Create a dynamic visible section
    let sec: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Malformed Resilience Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    println!("CREATED SEC ID: {}", sec.data.id);

    // 2. Create a valid text block in sec
    let b_valid: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec.data.id,
            "block_type": "text",
            "title": "Valid Block",
            "text": "Valid text content"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // 3. Manually insert a corrupt media asset with NULL storage_key attached to a text_image block directly in DB
    let corrupt_block_id = uuid::Uuid::new_v4();
    let corrupt_media_id = uuid::Uuid::new_v4();
    let page_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM pages WHERE slug = 'home'")
        .fetch_one(&harness.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO media_assets (id, media_type, storage_provider, storage_key, status) VALUES ($1, 'image', 'local', NULL, 'active')")
        .bind(corrupt_media_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content, sort_order, is_visible) VALUES ($1, $2, $3, 'corrupt-block', 'text_image', 'Corrupt Block', '{\"text\":\"Corrupt\"}', 99, TRUE)")
        .bind(corrupt_block_id)
        .bind(page_id.0)
        .bind(sec.data.id)
        .execute(&harness.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO section_media (section_id, media_asset_id) VALUES ($1, $2)")
        .bind(corrupt_block_id)
        .bind(corrupt_media_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    // 4. Query public page -> MUST return 200 OK with valid block present and corrupt block gracefully skipped
    let res = harness.client.get("/api/v1/public/page").dispatch().await;
    assert_eq!(res.status(), Status::Ok);

    let page_data: SingleResponse<PublicPageResponse> = res.into_json().await.unwrap();
    let sec_dto = page_data
        .data
        .sections
        .iter()
        .find(|s| s.id == sec.data.id)
        .expect("Section MUST appear");

    assert_eq!(
        sec_dto.blocks.len(),
        1,
        "Corrupt block with NULL storage key MUST be gracefully skipped"
    );
    assert_eq!(sec_dto.blocks[0].id, b_valid.data.id);

    // Cleanup
    common::cleanup_content_block(&harness.pool, b_valid.data.id)
        .await
        .ok();
    common::cleanup_content_block(&harness.pool, corrupt_block_id)
        .await
        .ok();
    sqlx::query("DELETE FROM media_assets WHERE id = $1")
        .bind(corrupt_media_id)
        .execute(&harness.pool)
        .await
        .ok();
    common::cleanup_spa_section(&harness.pool, sec.data.id)
        .await
        .ok();
}

#[tokio::test]
async fn test_real_multipart_image_and_video_upload_and_deactivation_matrix() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // 1. Create a normal active admin user
    let admin_id = Uuid::new_v4();
    let admin_email = format!("normal_admin_{}@example.com", Uuid::new_v4().simple());
    let password_hash = argon2::Argon2::default()
        .hash_password(
            b"AdminPassword123!",
            &argon2::password_hash::SaltString::generate(&mut rand::thread_rng()),
        )
        .unwrap()
        .to_string();

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, role, is_active) VALUES ($1, $2, $3, 'Normal Admin', 'admin', TRUE)",
    )
    .bind(admin_id)
    .bind(&admin_email)
    .bind(&password_hash)
    .execute(&harness.pool)
    .await
    .unwrap();

    // Authenticate as normal admin
    let login_res = harness
        .client
        .post("/api/v1/auth/login")
        .json(&serde_json::json!({
            "email": admin_email,
            "password": "AdminPassword123!"
        }))
        .dispatch()
        .await;
    assert_eq!(login_res.status(), Status::Ok);
    let login_data: SingleResponse<spa_sax_backend::application::dto::AuthTokensDto> =
        login_res.into_json().await.unwrap();
    let admin_token = login_data.data.access_token;

    // 2. Upload valid PNG image via multipart/form-data
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let boundary = "------------------------1234567890abcdef";
    let mut img_body = Vec::new();
    img_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    img_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"test_upload.png\"\r\n",
    );
    img_body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    img_body.extend_from_slice(&png_bytes);
    img_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let img_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(img_body)
        .dispatch()
        .await;

    assert_eq!(
        img_res.status(),
        Status::Ok,
        "Normal active admin image upload MUST succeed"
    );
    let img_dto: SingleResponse<AdminMediaDto> = img_res.into_json().await.unwrap();
    let (img_id, img_url) = match &img_dto.data {
        AdminMediaDto::Image { id, url, .. } => (*id, url.clone()),
        _ => panic!("Expected AdminMediaDto::Image"),
    };

    let img_storage_key = img_url
        .split("/uploads/")
        .nth(1)
        .expect("URL must contain /uploads/");
    let img_path = harness.temp_dir.path().join(img_storage_key);
    assert!(img_path.exists(), "Uploaded image file MUST exist on disk");

    // Cleanup img
    let _ = tokio::fs::remove_file(&img_path).await;
    common::cleanup_media(&harness.pool, img_id).await.ok();

    // 3. Upload valid MP4 video via multipart/form-data
    let mp4_bytes: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x1C, 0x66, 0x74, 0x79, 0x70, 0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02,
        0x00, 0x69, 0x73, 0x6F, 0x6D, 0x69, 0x73, 0x6F, 0x32, 0x61, 0x76, 0x63, 0x31,
    ];

    let mut vid_body = Vec::new();
    vid_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    vid_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"test_upload.mp4\"\r\n",
    );
    vid_body.extend_from_slice(b"Content-Type: video/mp4\r\n\r\n");
    vid_body.extend_from_slice(&mp4_bytes);
    vid_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let vid_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(vid_body)
        .dispatch()
        .await;

    assert_eq!(
        vid_res.status(),
        Status::Ok,
        "Normal active admin video upload MUST succeed"
    );
    let vid_dto: SingleResponse<AdminMediaDto> = vid_res.into_json().await.unwrap();
    let (vid_id, vid_url) = match &vid_dto.data {
        AdminMediaDto::Video { id, url, .. } => (*id, url.clone()),
        _ => panic!("Expected AdminMediaDto::Video"),
    };

    let vid_storage_key = vid_url
        .split("/uploads/")
        .nth(1)
        .expect("URL must contain /uploads/");
    let vid_path = harness.temp_dir.path().join(vid_storage_key);
    assert!(vid_path.exists(), "Uploaded video file MUST exist on disk");

    // Cleanup vid
    let _ = tokio::fs::remove_file(&vid_path).await;
    common::cleanup_media(&harness.pool, vid_id).await.ok();

    // 4. Super admin media upload regression
    let mut super_body = Vec::new();
    super_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    super_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"super.png\"\r\n",
    );
    super_body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    super_body.extend_from_slice(&png_bytes);
    super_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let super_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(super_body)
        .dispatch()
        .await;

    assert_eq!(super_res.status(), Status::Ok);
    let super_dto: SingleResponse<AdminMediaDto> = super_res.into_json().await.unwrap();
    let (super_id, super_url) = match &super_dto.data {
        AdminMediaDto::Image { id, url, .. } => (*id, url.clone()),
        _ => panic!("Expected AdminMediaDto::Image for super admin"),
    };
    let super_storage_key = super_url
        .split("/uploads/")
        .nth(1)
        .expect("URL must contain /uploads/");
    let _ = tokio::fs::remove_file(harness.temp_dir.path().join(super_storage_key)).await;
    common::cleanup_media(&harness.pool, super_id).await.ok();

    // 5. Inactive admin multipart upload regression
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(admin_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    let count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_assets")
        .fetch_one(&harness.pool)
        .await
        .unwrap();

    let mut inactive_body = Vec::new();
    inactive_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    inactive_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"inactive.png\"\r\n",
    );
    inactive_body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    inactive_body.extend_from_slice(&png_bytes);
    inactive_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let inactive_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(inactive_body)
        .dispatch()
        .await;

    assert!(
        inactive_res.status() == Status::Forbidden || inactive_res.status() == Status::Unauthorized,
        "Deactivated admin MUST be rejected with 403 Forbidden or 401 Unauthorized"
    );

    let count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_assets")
        .fetch_one(&harness.pool)
        .await
        .unwrap();
    assert_eq!(
        count_before.0, count_after.0,
        "No media asset MUST be created for inactive admin"
    );

    // Cleanup user
    common::cleanup_test_user(&harness.pool, admin_id)
        .await
        .ok();
}

#[tokio::test]
async fn test_public_spa_contract_reorder_move_delete_and_foreign_isolation_e2e() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;
    common::reset_home_sections_to_bootstrap(&harness.pool).await;

    let token = harness.super_admin_token.clone();

    // --- 1. Public Section Reorder E2E ---
    let init_res = harness.client.get("/api/v1/public/page").dispatch().await;
    assert_eq!(init_res.status(), Status::Ok);
    let init_data: SingleResponse<PublicPageResponse> = init_res.into_json().await.unwrap();
    let original_sec_ids: Vec<Uuid> = init_data.data.sections.iter().map(|s| s.id).collect();
    assert_eq!(original_sec_ids.len(), 5);

    let mut reversed_sec_ids = original_sec_ids.clone();
    reversed_sec_ids.reverse();

    let reorder_items: Vec<serde_json::Value> = reversed_sec_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| serde_json::json!({ "id": id, "sort_order": (idx as i32 + 1) * 10 }))
        .collect();

    let reorder_sec_res = harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "items": reorder_items }))
        .dispatch()
        .await;
    assert_eq!(reorder_sec_res.status(), Status::Ok);

    let pub_reorder_res = harness.client.get("/api/v1/public/page").dispatch().await;
    let pub_reorder_data: SingleResponse<PublicPageResponse> =
        pub_reorder_res.into_json().await.unwrap();
    let new_pub_sec_ids: Vec<Uuid> = pub_reorder_data
        .data
        .sections
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        new_pub_sec_ids, reversed_sec_ids,
        "Public section order MUST match new sort_order"
    );

    // Restore section order
    let restore_items: Vec<serde_json::Value> = original_sec_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| serde_json::json!({ "id": id, "sort_order": (idx as i32 + 1) * 10 }))
        .collect();

    harness
        .client
        .post("/api/v1/admin/spa-sections/reorder")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "items": restore_items }))
        .dispatch()
        .await;

    // --- 2. Public ContentBlock Reorder E2E ---
    let sec_reorder: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Block Reorder Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b_a: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_reorder.data.id,
            "block_type": "text",
            "title": "Block A",
            "text": "A"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b_b: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_reorder.data.id,
            "block_type": "text",
            "title": "Block B",
            "text": "B"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b_c: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_reorder.data.id,
            "block_type": "text",
            "title": "Block C",
            "text": "C"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Reorder A B C -> C A B
    let reorder_block_items = vec![
        serde_json::json!({ "id": b_c.data.id, "sort_order": 10 }),
        serde_json::json!({ "id": b_a.data.id, "sort_order": 20 }),
        serde_json::json!({ "id": b_b.data.id, "sort_order": 30 }),
    ];
    let reorder_block_res = harness
        .client
        .post(format!(
            "/api/v1/admin/spa-sections/{}/content-blocks/reorder",
            sec_reorder.data.id
        ))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "items": reorder_block_items }))
        .dispatch()
        .await;
    assert_eq!(reorder_block_res.status(), Status::Ok);

    let pub_block_page: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let sec_in_pub = pub_block_page
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_reorder.data.id)
        .unwrap();
    let pub_block_ids: Vec<Uuid> = sec_in_pub.blocks.iter().map(|b| b.id).collect();
    assert_eq!(pub_block_ids, vec![b_c.data.id, b_a.data.id, b_b.data.id]);

    // Cleanup blocks & sec
    common::cleanup_content_block(&harness.pool, b_a.data.id)
        .await
        .ok();
    common::cleanup_content_block(&harness.pool, b_b.data.id)
        .await
        .ok();
    common::cleanup_content_block(&harness.pool, b_c.data.id)
        .await
        .ok();
    common::cleanup_spa_section(&harness.pool, sec_reorder.data.id)
        .await
        .ok();

    // --- 3. Public Move E2E ---
    let sec_p: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Partners Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let sec_aw: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Awards Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b_move: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_p.data.id,
            "block_type": "text",
            "title": "Move Block",
            "text": "Move text"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Verify before move
    let pub_before_move: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    let p_before = pub_before_move
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_p.data.id)
        .unwrap();
    let aw_before = pub_before_move
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_aw.data.id)
        .unwrap();
    assert_eq!(p_before.blocks.len(), 1);
    assert_eq!(aw_before.blocks.len(), 0);

    // Move to Awards Sec
    let patch_move_res = harness
        .client
        .patch(format!("/api/v1/admin/content-blocks/{}", b_move.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "spa_section_id": sec_aw.data.id }))
        .dispatch()
        .await;
    assert_eq!(patch_move_res.status(), Status::Ok);

    // Verify after move
    let pub_after_move: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    let p_after = pub_after_move
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_p.data.id)
        .unwrap();
    let aw_after = pub_after_move
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_aw.data.id)
        .unwrap();
    assert_eq!(p_after.blocks.len(), 0);
    assert_eq!(aw_after.blocks.len(), 1);
    assert_eq!(aw_after.blocks[0].id, b_move.data.id);

    common::cleanup_content_block(&harness.pool, b_move.data.id)
        .await
        .ok();
    common::cleanup_spa_section(&harness.pool, sec_p.data.id)
        .await
        .ok();
    common::cleanup_spa_section(&harness.pool, sec_aw.data.id)
        .await
        .ok();

    // --- 4. Deleted ContentBlock & Deleted SpaSection Public E2E ---
    let sec_del: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Del Sec Test" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let b_del: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_del.data.id,
            "block_type": "text",
            "title": "Del Block",
            "text": "Del text"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Delete block
    let del_b_res = harness
        .client
        .delete(format!("/api/v1/admin/content-blocks/{}", b_del.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(del_b_res.status(), Status::Ok);

    // Section visible with blocks = []
    let pub_after_del_b: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    let sec_del_dto = pub_after_del_b
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_del.data.id)
        .unwrap();
    assert_eq!(sec_del_dto.blocks.len(), 0);

    // Delete empty section
    let del_sec_res = harness
        .client
        .delete(format!("/api/v1/admin/spa-sections/{}", sec_del.data.id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(del_sec_res.status(), Status::Ok);

    // Section completely absent
    let pub_after_del_sec: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert!(pub_after_del_sec
        .data
        .sections
        .iter()
        .all(|s| s.id != sec_del.data.id));

    common::cleanup_content_block(&harness.pool, b_del.data.id)
        .await
        .ok();
    common::cleanup_spa_section(&harness.pool, sec_del.data.id)
        .await
        .ok();

    // --- 5. Foreign Page Public Isolation E2E ---
    let foreign_page_id = Uuid::new_v4();
    let foreign_sec_id = Uuid::new_v4();
    let foreign_block_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO pages (id, slug, title) VALUES ($1, 'foreign-test-page', 'Foreign Test Page')",
    )
    .bind(foreign_page_id)
    .execute(&harness.pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible) VALUES ($1, $2, 'foreign-sec', 'Foreign Sec', 'Foreign Sec', 10, TRUE)")
        .bind(foreign_sec_id)
        .bind(foreign_page_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO sections (id, page_id, spa_section_id, section_key, section_type, title, content, sort_order, is_visible) VALUES ($1, $2, $3, 'foreign-block', 'text', 'Foreign Block', '{\"text\":\"Foreign\"}', 10, TRUE)")
        .bind(foreign_block_id)
        .bind(foreign_page_id)
        .bind(foreign_sec_id)
        .execute(&harness.pool)
        .await
        .unwrap();

    let pub_foreign_res: SingleResponse<PublicPageResponse> = harness
        .client
        .get("/api/v1/public/page")
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(pub_foreign_res.data.page.slug, "home");
    assert!(pub_foreign_res
        .data
        .sections
        .iter()
        .all(|s| s.id != foreign_sec_id));
    for s in &pub_foreign_res.data.sections {
        assert!(s.blocks.iter().all(|b| b.id != foreign_block_id));
    }

    // Cleanup foreign
    sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(foreign_block_id)
        .execute(&harness.pool)
        .await
        .ok();
    sqlx::query("DELETE FROM spa_sections WHERE id = $1")
        .bind(foreign_sec_id)
        .execute(&harness.pool)
        .await
        .ok();
    sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(foreign_page_id)
        .execute(&harness.pool)
        .await
        .ok();

    // --- 6. Public Image, Video & YouTube DTO E2E ---
    let sec_media: SingleResponse<AdminSpaSectionDto> = harness
        .client
        .post("/api/v1/admin/spa-sections")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({ "title": "Public Media DTO Sec" }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Upload real PNG
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let boundary = "------------------------1234567890media";
    let mut img_body = Vec::new();
    img_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    img_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"pub_img.png\"\r\n",
    );
    img_body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    img_body.extend_from_slice(&png_bytes);
    img_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let img_up_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(img_body)
        .dispatch()
        .await;
    assert_eq!(img_up_res.status(), Status::Ok);
    let img_media_dto: SingleResponse<AdminMediaDto> = img_up_res.into_json().await.unwrap();
    let (img_id, img_url) = match &img_media_dto.data {
        AdminMediaDto::Image { id, url, .. } => (*id, url.clone()),
        _ => panic!("Expected AdminMediaDto::Image"),
    };

    // Attach image to block
    let b_img: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_media.data.id,
            "block_type": "text_image",
            "title": "Image Block",
            "text": "Image text",
            "media_id": img_id
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Upload real MP4 video
    let mp4_bytes: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x1C, 0x66, 0x74, 0x79, 0x70, 0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02,
        0x00, 0x69, 0x73, 0x6F, 0x6D, 0x69, 0x73, 0x6F, 0x32, 0x61, 0x76, 0x63, 0x31,
    ];
    let mut vid_body = Vec::new();
    vid_body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    vid_body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"pub_vid.mp4\"\r\n",
    );
    vid_body.extend_from_slice(b"Content-Type: video/mp4\r\n\r\n");
    vid_body.extend_from_slice(&mp4_bytes);
    vid_body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let vid_up_res = harness
        .client
        .post("/api/v1/admin/media/upload")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .header(Header::new(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .body(vid_body)
        .dispatch()
        .await;
    assert_eq!(vid_up_res.status(), Status::Ok);
    let vid_media_dto: SingleResponse<AdminMediaDto> = vid_up_res.into_json().await.unwrap();
    let (vid_id, vid_url) = match &vid_media_dto.data {
        AdminMediaDto::Video { id, url, .. } => (*id, url.clone()),
        _ => panic!("Expected AdminMediaDto::Video"),
    };

    let b_vid: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_media.data.id,
            "block_type": "text_video",
            "title": "Video Block",
            "text": "Video text",
            "media_id": vid_id
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // YouTube media
    let yt_res: SingleResponse<AdminMediaDto> = harness
        .client
        .post("/api/v1/admin/media/youtube")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "youtube_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "title": "Rickroll"
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    let yt_id = match &yt_res.data {
        AdminMediaDto::Youtube { id, .. } => *id,
        _ => panic!("Expected AdminMediaDto::Youtube"),
    };

    let b_yt: SingleResponse<AdminContentBlockDto> = harness
        .client
        .post("/api/v1/admin/content-blocks")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .json(&serde_json::json!({
            "spa_section_id": sec_media.data.id,
            "block_type": "text_youtube",
            "title": "YouTube Block",
            "text": "YouTube text",
            "media_id": yt_id
        }))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();

    // Fetch public page and verify media DTO shapes
    let pub_media_page_res = harness.client.get("/api/v1/public/page").dispatch().await;
    assert_eq!(pub_media_page_res.status(), Status::Ok);
    let raw_pub_json = pub_media_page_res.into_string().await.unwrap();
    let pub_media_page: SingleResponse<PublicPageResponse> =
        serde_json::from_str(&raw_pub_json).unwrap();

    let sec_pub_media = pub_media_page
        .data
        .sections
        .iter()
        .find(|s| s.id == sec_media.data.id)
        .unwrap();
    assert_eq!(sec_pub_media.blocks.len(), 3);

    let pub_b_img = sec_pub_media
        .blocks
        .iter()
        .find(|b| b.id == b_img.data.id)
        .unwrap();
    let pub_img_m = pub_b_img
        .media
        .as_ref()
        .expect("Public image media MUST exist");
    match pub_img_m {
        PublicMediaDto::Image { id, url, .. } => {
            assert_eq!(*id, img_id);
            assert!(!url.is_empty());
        }
        _ => panic!("Expected PublicMediaDto::Image"),
    }

    // Verify storage_key, filesystem path, created_by are NOT in raw JSON
    assert!(!raw_pub_json.contains("storage_key"));
    assert!(!raw_pub_json.contains("created_by"));
    assert!(!raw_pub_json.contains("deleted_at"));

    // Verify PublicMediaDto::Video for text_video block
    let pub_b_vid = sec_pub_media
        .blocks
        .iter()
        .find(|b| b.id == b_vid.data.id)
        .unwrap();
    assert_eq!(
        pub_b_vid.block_type,
        spa_sax_backend::domain::sections::ContentBlockType::TextVideo
    );
    let pub_vid_m = pub_b_vid
        .media
        .as_ref()
        .expect("Public video media MUST exist");
    match pub_vid_m {
        PublicMediaDto::Video { id, url, .. } => {
            assert_eq!(
                *id, vid_id,
                "Media identity MUST equal uploaded MediaAsset ID"
            );
            assert!(!url.is_empty(), "Public video URL MUST be non-empty");
            assert!(
                url.contains("/uploads/"),
                "Public video URL MUST be an HTTP-facing storage URL"
            );
        }
        _ => panic!("Expected PublicMediaDto::Video"),
    }

    let pub_b_yt = sec_pub_media
        .blocks
        .iter()
        .find(|b| b.id == b_yt.data.id)
        .unwrap();
    let pub_yt_m = pub_b_yt
        .media
        .as_ref()
        .expect("Public YouTube media MUST exist");
    match pub_yt_m {
        PublicMediaDto::Youtube {
            youtube_video_id,
            embed_url,
            ..
        } => {
            assert_eq!(youtube_video_id, "dQw4w9WgXcQ");
            assert!(embed_url.contains("dQw4w9WgXcQ"));
        }
        _ => panic!("Expected PublicMediaDto::Youtube"),
    }

    // Cleanup files, blocks, media, section
    let img_storage_key = img_url.split("/uploads/").nth(1).unwrap();
    let _ = tokio::fs::remove_file(harness.temp_dir.path().join(img_storage_key)).await;
    let vid_storage_key = vid_url.split("/uploads/").nth(1).unwrap();
    let _ = tokio::fs::remove_file(harness.temp_dir.path().join(vid_storage_key)).await;

    common::cleanup_content_block(&harness.pool, b_img.data.id)
        .await
        .ok();
    common::cleanup_content_block(&harness.pool, b_vid.data.id)
        .await
        .ok();
    common::cleanup_content_block(&harness.pool, b_yt.data.id)
        .await
        .ok();
    common::cleanup_media(&harness.pool, img_id).await.ok();
    common::cleanup_media(&harness.pool, vid_id).await.ok();
    common::cleanup_media(&harness.pool, yt_id).await.ok();
    common::cleanup_spa_section(&harness.pool, sec_media.data.id)
        .await
        .ok();

    common::reset_home_sections_to_bootstrap(&harness.pool).await;
}

#[tokio::test]
async fn test_public_openapi_get_operation_and_schema_regression() {
    use utoipa::openapi::path::PathItemType;
    use utoipa::OpenApi;

    let openapi = spa_sax_backend::bootstrap::ApiDoc::openapi();
    let paths = &openapi.paths;

    let public_path = paths
        .paths
        .get("/api/v1/public/page")
        .expect("OpenAPI missing /api/v1/public/page path");

    let get_op = public_path
        .operations
        .get(&PathItemType::Get)
        .expect("OpenAPI /api/v1/public/page MUST have GET operation");

    // Public endpoint MUST NOT require admin security auth
    assert!(
        get_op.security.is_none() || get_op.security.as_ref().unwrap().is_empty(),
        "Public endpoint GET /api/v1/public/page MUST NOT require admin security auth"
    );

    let schemas = &openapi
        .components
        .as_ref()
        .expect("OpenAPI missing components")
        .schemas;
    assert!(
        schemas.contains_key("PublicPageResponse"),
        "OpenAPI missing PublicPageResponse schema"
    );
    assert!(
        schemas.contains_key("PublicPageDto"),
        "OpenAPI missing PublicPageDto schema"
    );
    assert!(
        schemas.contains_key("PublicSpaSectionDto"),
        "OpenAPI missing PublicSpaSectionDto schema"
    );
    assert!(
        schemas.contains_key("PublicContentBlockDto"),
        "OpenAPI missing PublicContentBlockDto schema"
    );
    assert!(
        schemas.contains_key("PublicMediaDto"),
        "OpenAPI missing PublicMediaDto schema"
    );
}

#[tokio::test]
async fn test_media_service_storage_failure_compensating_delete() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let failing_storage = spa_sax_backend::infrastructure::storage::LocalStorageProvider::new(
        "/proc/invalid_non_existent_dir_for_test",
        "http://localhost:8000/uploads".to_string(),
    );

    let media_service = spa_sax_backend::application::services::media_service::MediaService::new(
        &harness.pool,
        &harness.config,
        std::sync::Arc::new(failing_storage),
    );

    let auth = spa_sax_backend::api::guards::AuthenticatedUser {
        id: harness.super_admin_id,
        email: harness.super_admin_email.clone(),
        display_name: "Super Admin".to_string(),
        role: spa_sax_backend::domain::users::Role::SuperAdmin,
        is_active: true,
    };

    let temp_jpg_path = harness.temp_dir.path().join("comp_test.jpg");
    std::fs::write(
        &temp_jpg_path,
        [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46],
    )
    .unwrap();

    let upload_res = media_service
        .process_uploaded_file(
            &auth,
            &temp_jpg_path,
            Some("comp_test.jpg"),
            "image/jpeg",
            8,
        )
        .await;

    assert!(
        upload_res.is_err(),
        "Storage write MUST return error when move_file fails"
    );

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM media_assets WHERE original_filename = 'comp_test.jpg'",
    )
    .fetch_one(&harness.pool)
    .await
    .unwrap();

    assert_eq!(
        count.0, 0,
        "Compensating DB cleanup MUST delete DB row when storage write fails"
    );
}

#[tokio::test]
async fn test_user_update_full_lifecycle_and_validation() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let target_email = format!("update_target_{}@example.com", Uuid::new_v4().simple());

    // 1. Create a target user to update
    let create_res = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": target_email,
            "password": "TargetPassword123!",
            "display_name": "Initial Name",
            "role": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(create_res.status(), Status::Ok);
    let create_body: SingleResponse<UserDto> = create_res.into_json().await.unwrap();
    let target_id = create_body.data.id;

    // 2. PATCH display_name, role, and is_active
    let patch_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", target_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "display_name": "Updated Display Name",
            "role": "super_admin",
            "is_active": false
        }))
        .dispatch()
        .await;

    assert_eq!(patch_res.status(), Status::Ok);
    let patch_body: SingleResponse<UserDto> = patch_res.into_json().await.unwrap();
    assert_eq!(patch_body.data.display_name, "Updated Display Name");
    assert_eq!(patch_body.data.role, "super_admin");
    assert!(!patch_body.data.is_active);

    // 3. Partial PATCH (only display_name)
    let partial_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", target_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "display_name": "Second Updated Name"
        }))
        .dispatch()
        .await;

    assert_eq!(partial_res.status(), Status::Ok);
    let partial_body: SingleResponse<UserDto> = partial_res.into_json().await.unwrap();
    assert_eq!(partial_body.data.display_name, "Second Updated Name");
    assert_eq!(partial_body.data.role, "super_admin"); // Unchanged
    assert!(!partial_body.data.is_active); // Unchanged

    // 4. Empty payload returns 422 Validation Error
    let empty_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", target_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({}))
        .dispatch()
        .await;

    assert_eq!(empty_res.status(), Status::UnprocessableEntity);

    // 5. Invalid role returns 422 Validation Error
    let invalid_role_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", target_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "role": "invalid_role"
        }))
        .dispatch()
        .await;

    assert_eq!(invalid_role_res.status(), Status::UnprocessableEntity);

    // 6. Unknown UUID returns 404 User Not Found
    let random_id = Uuid::new_v4();
    let unknown_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", random_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "display_name": "Ghost User"
        }))
        .dispatch()
        .await;

    assert_eq!(unknown_res.status(), Status::NotFound);
}

#[tokio::test]
async fn test_self_deactivation_and_self_demotion_guards() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    // 1. PATCH self deactivation returns 409 Conflict
    let patch_self_deactivate = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", harness.super_admin_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "is_active": false
        }))
        .dispatch()
        .await;

    assert_eq!(patch_self_deactivate.status(), Status::Conflict);

    // 2. POST /deactivate self deactivation returns 409 Conflict
    let post_self_deactivate = harness
        .client
        .post(format!(
            "/api/v1/admin/users/{}/deactivate",
            harness.super_admin_id
        ))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .dispatch()
        .await;

    assert_eq!(post_self_deactivate.status(), Status::Conflict);

    let second_email = format!("super_admin_2_{}@example.com", Uuid::new_v4().simple());

    // 3. Create a second super_admin so super_admin count > 1
    let second_admin_res = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": second_email,
            "password": "SuperAdminPass123!",
            "display_name": "Second Super Admin",
            "role": "super_admin"
        }))
        .dispatch()
        .await;
    assert_eq!(second_admin_res.status(), Status::Ok);

    // 4. Self demotion to admin succeeds when active super_admin count > 1
    let patch_self_demote = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", harness.super_admin_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "role": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(patch_self_demote.status(), Status::Ok);
    let demote_body: SingleResponse<UserDto> = patch_self_demote.into_json().await.unwrap();
    assert_eq!(demote_body.data.role, "admin");

    // Restore super_admin role in DB for harness super_admin user fixture
    sqlx::query("UPDATE users SET role = 'super_admin' WHERE id = $1")
        .bind(harness.super_admin_id)
        .execute(&harness.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_super_admin_create_both_admin_and_super_admin_roles() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let admin_email = format!("role_admin_{}@example.com", Uuid::new_v4().simple());
    let super_email = format!("role_super_{}@example.com", Uuid::new_v4().simple());

    // 1. Create admin role user
    let res_admin = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": admin_email,
            "password": "RolePass123!",
            "display_name": "Role Admin",
            "role": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(res_admin.status(), Status::Ok);
    let body_admin: SingleResponse<UserDto> = res_admin.into_json().await.unwrap();
    assert_eq!(body_admin.data.role, "admin");

    // 2. Create super_admin role user
    let res_super = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": super_email,
            "password": "RolePass123!",
            "display_name": "Role Super Admin",
            "role": "super_admin"
        }))
        .dispatch()
        .await;

    assert_eq!(res_super.status(), Status::Ok);
    let body_super: SingleResponse<UserDto> = res_super.into_json().await.unwrap();
    assert_eq!(body_super.data.role, "super_admin");
}

#[tokio::test]
async fn test_normal_admin_rejected_for_all_user_operations() {
    let _lock = DB_LOCK.lock().await;
    let harness = TestHarness::new().await;

    let normal_admin_email = format!("normal_admin_{}@example.com", Uuid::new_v4().simple());
    let forbidden_create_email =
        format!("forbidden_create_{}@example.com", Uuid::new_v4().simple());

    // 1. Create a normal admin user
    let admin_res = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", harness.super_admin_token),
        ))
        .json(&json!({
            "email": normal_admin_email,
            "password": "AdminPassword123!",
            "display_name": "Normal Admin Users",
            "role": "admin"
        }))
        .dispatch()
        .await;
    assert_eq!(admin_res.status(), Status::Ok);

    // Login as normal admin
    let login_res = harness
        .client
        .post("/api/v1/auth/login")
        .json(&json!({
            "email": normal_admin_email,
            "password": "AdminPassword123!"
        }))
        .dispatch()
        .await;
    assert_eq!(login_res.status(), Status::Ok);
    let login_body: SingleResponse<spa_sax_backend::application::dto::AuthTokensDto> =
        login_res.into_json().await.unwrap();
    let admin_token = login_body.data.access_token;

    // 2. Normal admin GET /users -> 403 Forbidden
    let list_res = harness
        .client
        .get("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(list_res.status(), Status::Forbidden);

    // 3. Normal admin POST /users -> 403 Forbidden
    let create_res = harness
        .client
        .post("/api/v1/admin/users")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .json(&json!({
            "email": forbidden_create_email,
            "password": "Password123!",
            "display_name": "Forbidden"
        }))
        .dispatch()
        .await;
    assert_eq!(create_res.status(), Status::Forbidden);

    // 4. Normal admin PATCH /users/:id -> 403 Forbidden
    let patch_res = harness
        .client
        .patch(format!("/api/v1/admin/users/{}", harness.super_admin_id))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .json(&json!({
            "display_name": "Hacked Name"
        }))
        .dispatch()
        .await;
    assert_eq!(patch_res.status(), Status::Forbidden);

    // 5. Normal admin POST /users/:id/activate -> 403 Forbidden
    let activate_res = harness
        .client
        .post(format!(
            "/api/v1/admin/users/{}/activate",
            harness.super_admin_id
        ))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(activate_res.status(), Status::Forbidden);

    // 6. Normal admin POST /users/:id/deactivate -> 403 Forbidden
    let deactivate_res = harness
        .client
        .post(format!(
            "/api/v1/admin/users/{}/deactivate",
            harness.super_admin_id
        ))
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", admin_token),
        ))
        .dispatch()
        .await;
    assert_eq!(deactivate_res.status(), Status::Forbidden);
}
