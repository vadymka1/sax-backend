use spa_sax_backend::domain::media::YoutubeUrlParser;
use spa_sax_backend::domain::users::Role;
use spa_sax_backend::infrastructure::auth::{PasswordService, TokenService};
use spa_sax_backend::infrastructure::storage::LocalStorageProvider;
use spa_sax_backend::shared::pagination::PaginationMeta;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing_and_verification() {
        let password = "SuperSecretPassword123!";
        let hash = PasswordService::hash_password(password).expect("Failed to hash password");

        assert!(PasswordService::verify_password(password, &hash));
        assert!(!PasswordService::verify_password("WrongPassword!", &hash));
    }

    #[test]
    fn test_jwt_generation_and_decoding() {
        let user_id = uuid::Uuid::new_v4();
        let email = "admin@example.com";
        let role = Role::Admin;
        let secret = "test_secret_key_12345678901234567890";
        let ttl = 900;

        let token = TokenService::generate_access_token(user_id, email, role, secret, ttl)
            .expect("Failed to generate token");

        let claims = TokenService::decode_access_token(&token, secret)
            .expect("Failed to decode valid token");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.email, email);
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_refresh_token_generation_256bits() {
        let token1 = TokenService::generate_refresh_token();
        let token2 = TokenService::generate_refresh_token();

        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 64); // 32 bytes (256 bits) encoded as hex
        assert_eq!(token2.len(), 64);

        let hash1 = TokenService::hash_refresh_token(&token1);
        let hash2 = TokenService::hash_refresh_token(&token1);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_role_hierarchy_permissions() {
        assert!(Role::SuperAdmin.can_manage_users());
        assert!(!Role::Admin.can_manage_users());
        assert!(!Role::Editor.can_manage_users());

        assert!(Role::Admin.can_manage_content());
        assert!(Role::Admin.can_manage_media());

        assert!(Role::SuperAdmin.can_create_role(Role::Admin));
        assert!(Role::SuperAdmin.can_create_role(Role::SuperAdmin));
        assert!(!Role::Admin.can_create_role(Role::SuperAdmin));
    }

    #[test]
    fn test_storage_path_sanitization_rejects_traversal() {
        let provider =
            LocalStorageProvider::new("./uploads", "http://localhost:8000/uploads".to_string());

        assert!(provider.sanitize_path("images/2026/08/file.jpg").is_ok());
        assert!(provider.sanitize_path("../etc/passwd").is_err());
        assert!(provider.sanitize_path("/absolute/path").is_err());
    }

    #[test]
    fn test_youtube_url_parsing_variants() {
        let valid_urls = vec![
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://youtu.be/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/embed/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
        ];

        for (url, expected_id) in valid_urls {
            let id = YoutubeUrlParser::parse_id(url).expect("Should parse valid YouTube URL");
            assert_eq!(id, expected_id);
        }
    }

    #[test]
    fn test_magic_bytes_detection_and_rejection() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        // 1. Valid JPEG Magic Bytes (FF D8 FF)
        let jpeg_path = temp_dir.path().join("test.jpg");
        std::fs::write(&jpeg_path, [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46]).unwrap();
        let kind = infer::get_from_path(&jpeg_path)
            .unwrap()
            .expect("Infer failed");
        assert_eq!(kind.mime_type(), "image/jpeg");

        // 2. Fake JPEG (HTML file renamed to .jpg)
        let fake_jpg_path = temp_dir.path().join("fake.jpg");
        std::fs::write(
            &fake_jpg_path,
            b"<html><body>Executable Payload</body></html>",
        )
        .unwrap();
        let kind_fake = infer::get_from_path(&fake_jpg_path).unwrap();
        assert!(kind_fake.is_none() || kind_fake.unwrap().mime_type() != "image/jpeg");

        // 3. SVG file (must not be detected as image/jpeg or png)
        let svg_path = temp_dir.path().join("test.svg");
        std::fs::write(
            &svg_path,
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        )
        .unwrap();
        let kind_svg = infer::get_from_path(&svg_path).unwrap();
        assert!(kind_svg.is_none() || kind_svg.unwrap().mime_type() != "image/jpeg");
    }

    #[test]
    fn test_pagination_meta_calculation() {
        let meta = PaginationMeta::new(1, 20, 45);
        assert_eq!(meta.total_pages, 3);
        assert_eq!(meta.page, 1);
        assert_eq!(meta.page_size, 20);

        let meta2 = PaginationMeta::new(3, 20, 45);
        assert_eq!(meta2.total_pages, 3);
        assert_eq!(meta2.page, 3);
    }

    #[test]
    fn test_database_safety_validator_rules() {
        use spa_sax_backend::config::validate_test_database_environment;

        // 1. Accepted cases
        assert!(validate_test_database_environment(
            "test",
            "postgres://app:pass@localhost:5432/app_test"
        )
        .is_ok());
        assert!(validate_test_database_environment(
            "test",
            "postgres://user:pass@localhost:5432/spa_backend_test"
        )
        .is_ok());
        assert!(validate_test_database_environment(
            "test",
            "postgres://user:pass@localhost:5432/test_spa_backend"
        )
        .is_ok());
        assert!(validate_test_database_environment(
            "test",
            "postgres://user:pass@localhost:5432/integration_test"
        )
        .is_ok());

        // 2. Rejected: wrong APP_ENV
        assert!(validate_test_database_environment(
            "development",
            "postgres://app:pass@localhost:5432/app_test"
        )
        .is_err());
        assert!(validate_test_database_environment(
            "production",
            "postgres://app:pass@localhost:5432/app_test"
        )
        .is_err());

        // 3. Rejected: unsafe DB names
        assert!(validate_test_database_environment(
            "test",
            "postgres://app:pass@localhost:5432/app_db"
        )
        .is_err());
        assert!(validate_test_database_environment(
            "test",
            "postgres://app:pass@localhost:5432/production"
        )
        .is_err());
        assert!(validate_test_database_environment(
            "test",
            "postgres://app:pass@localhost:5432/spa_backend"
        )
        .is_err());

        // 4. Rejected: username contains test but DB is app_db
        assert!(validate_test_database_environment(
            "test",
            "postgres://test_user:password@localhost:5432/app_db"
        )
        .is_err());

        // 5. Rejected: query param contains test but DB is app_db
        assert!(validate_test_database_environment(
            "test",
            "postgres://user:pass@localhost:5432/app_db?test=1"
        )
        .is_err());

        // 6. Rejected: malformed URL
        assert!(validate_test_database_environment("test", "not_a_valid_postgres_url").is_err());
    }

    #[test]
    fn test_openapi_spec_structure_and_security_compliance() {
        use spa_sax_backend::bootstrap::ApiDoc;
        use utoipa::OpenApi;

        let openapi = ApiDoc::openapi();

        // 1. Verify OpenAPI Metadata
        assert_eq!(openapi.info.title, "SPA Sax Backend API");
        assert_eq!(openapi.info.version, env!("CARGO_PKG_VERSION"));
        assert!(!openapi
            .info
            .description
            .clone()
            .unwrap_or_default()
            .is_empty());

        // 2. Verify Tags
        let tags = openapi.tags.as_ref().expect("Tags must be present");
        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        let expected_tags = vec![
            "Auth",
            "Admin Users",
            "SPA Sections",
            "Content Blocks",
            "Media",
            "Public",
            "Health",
        ];
        for expected in expected_tags {
            assert!(
                tag_names.contains(&expected),
                "Missing expected tag: {}",
                expected
            );
        }

        // 3. Verify Security Scheme (bearer_auth)
        let components = openapi.components.expect("Components must be defined");
        let bearer_scheme = components
            .security_schemes
            .get("bearer_auth")
            .expect("bearer_auth security scheme must be present");

        match bearer_scheme {
            utoipa::openapi::security::SecurityScheme::Http(http_auth) => {
                assert!(
                    matches!(
                        http_auth.scheme,
                        utoipa::openapi::security::HttpAuthScheme::Bearer
                    ),
                    "Security scheme must be Bearer"
                );
                assert_eq!(http_auth.bearer_format.as_deref(), Some("JWT"));
            }
            _ => panic!("bearer_auth must be an HTTP Bearer scheme"),
        }

        // 4. Verify Registered Paths (21 path patterns, total 29 route operations)
        let paths = &openapi.paths;
        let expected_paths = vec![
            "/health",
            "/health/live",
            "/health/ready",
            "/api/v1/auth/login",
            "/api/v1/auth/refresh",
            "/api/v1/auth/logout",
            "/api/v1/auth/me",
            "/api/v1/admin/users",
            "/api/v1/admin/users/{id}",
            "/api/v1/admin/users/{id}/activate",
            "/api/v1/admin/users/{id}/deactivate",
            "/api/v1/admin/spa-sections",
            "/api/v1/admin/spa-sections/{id}",
            "/api/v1/admin/spa-sections/reorder",
            "/api/v1/admin/spa-sections/{spa_section_id}/content-blocks/reorder",
            "/api/v1/admin/content-blocks",
            "/api/v1/admin/content-blocks/{id}",
            "/api/v1/admin/media/upload",
            "/api/v1/admin/media/youtube",
            "/api/v1/admin/media",
            "/api/v1/admin/media/{id}",
            "/api/v1/public/page",
        ];

        for path in expected_paths {
            assert!(
                paths.paths.contains_key(path),
                "Missing expected OpenAPI path: {}",
                path
            );
        }

        // 5. Verify Protected Endpoints Require bearer_auth
        let protected_paths = vec![
            "/api/v1/auth/me",
            "/api/v1/admin/users",
            "/api/v1/admin/spa-sections",
            "/api/v1/admin/content-blocks",
            "/api/v1/admin/media",
        ];

        for path_key in protected_paths {
            let path_item = paths.paths.get(path_key).unwrap();
            for op in path_item.operations.values() {
                let sec_reqs = op
                    .security
                    .as_ref()
                    .expect("Protected operation must have security requirement");
                let has_bearer = sec_reqs.iter().any(|s| {
                    serde_json::to_string(s)
                        .unwrap_or_default()
                        .contains("bearer_auth")
                });
                assert!(
                    has_bearer,
                    "Operation on {} missing bearer_auth requirement",
                    path_key
                );
            }
        }

        // 6. Verify Public & Health Endpoints Do NOT Require bearer_auth
        let public_paths = vec![
            "/api/v1/public/page",
            "/health",
            "/health/live",
            "/health/ready",
        ];

        for path_key in public_paths {
            let path_item = paths.paths.get(path_key).unwrap();
            for op in path_item.operations.values() {
                assert!(
                    op.security.is_none() || op.security.as_ref().unwrap().is_empty(),
                    "Public path {} must NOT have security requirement",
                    path_key
                );
            }
        }

        // 7. Verify Schema Registrations
        let schemas = &components.schemas;
        let required_schemas = vec![
            "UserDto",
            "CreateUserRequest",
            "UpdateUserRequest",
            "AuthTokensDto",
            "RefreshTokenDataDto",
            "MessageDataDto",
            "PublicPageResponse",
            "ApiErrorResponse",
            "Role",
            "ContentBlockType",
        ];

        for schema_name in required_schemas {
            assert!(
                schemas.contains_key(schema_name),
                "Missing required component schema: {}",
                schema_name
            );
        }
    }

    #[test]
    fn test_users_patch_openapi_contract_regression() {
        use spa_sax_backend::bootstrap::ApiDoc;
        use utoipa::openapi::path::PathItemType;
        use utoipa::OpenApi;

        let openapi = ApiDoc::openapi();
        let path_item = openapi
            .paths
            .paths
            .get("/api/v1/admin/users/{id}")
            .expect("Path /api/v1/admin/users/{id} must exist in OpenAPI spec");

        // 1. Assert PATCH operation exists
        assert!(
            path_item.operations.contains_key(&PathItemType::Patch),
            "PATCH operation must exist on /api/v1/admin/users/{{id}}"
        );

        // 2. Assert phantom GET and DELETE operations are absent
        assert!(
            !path_item.operations.contains_key(&PathItemType::Get),
            "Phantom GET operation must NOT exist on /api/v1/admin/users/{{id}}"
        );
        assert!(
            !path_item.operations.contains_key(&PathItemType::Delete),
            "Phantom DELETE operation must NOT exist on /api/v1/admin/users/{{id}}"
        );

        let patch_op = path_item.operations.get(&PathItemType::Patch).unwrap();

        // 3. Assert PATCH security requirement (bearer_auth)
        let sec_reqs = patch_op
            .security
            .as_ref()
            .expect("PATCH operation must have security requirement");
        let has_bearer = sec_reqs.iter().any(|s| {
            serde_json::to_string(s)
                .unwrap_or_default()
                .contains("bearer_auth")
        });
        assert!(
            has_bearer,
            "PATCH operation missing bearer_auth requirement"
        );

        // 4. Assert UpdateUserRequest schema reference in request body
        let req_body = patch_op
            .request_body
            .as_ref()
            .expect("PATCH operation must have request body");
        let req_body_str = serde_json::to_string(req_body).unwrap_or_default();
        assert!(
            req_body_str.contains("UpdateUserRequest"),
            "Request body must reference UpdateUserRequest schema"
        );

        // 5. Assert status codes documented on PATCH
        let responses = &patch_op.responses.responses;
        let expected_statuses = vec!["200", "401", "403", "404", "409", "422"];
        for status in expected_statuses {
            assert!(
                responses.contains_key(status),
                "PATCH /api/v1/admin/users/{{id}} missing documented status {}",
                status
            );
        }

        // 6. Assert POST /api/v1/admin/users documents 422 for validation
        let create_path = openapi
            .paths
            .paths
            .get("/api/v1/admin/users")
            .expect("Path /api/v1/admin/users must exist");
        let post_op = create_path
            .operations
            .get(&PathItemType::Post)
            .expect("POST operation must exist on /api/v1/admin/users");
        assert!(
            post_op.responses.responses.contains_key("422"),
            "POST /api/v1/admin/users must document 422 for validation errors"
        );
    }

    #[tokio::test]
    async fn test_swagger_runtime_mounting_development() {
        use rocket::http::Status;
        use rocket::local::asynchronous::Client;
        use spa_sax_backend::bootstrap::mount_swagger;

        let rocket = mount_swagger(rocket::build(), "development");
        let client = Client::untracked(rocket)
            .await
            .expect("Failed to build Rocket client");

        // 1. GET /swagger-ui/ returns HTTP 200 OK
        let res_ui = client.get("/swagger-ui/").dispatch().await;
        assert_eq!(res_ui.status(), Status::Ok);
        let body_ui = res_ui.into_string().await.unwrap_or_default();
        assert!(body_ui.contains("swagger-ui") || body_ui.contains("html"));

        // 2. GET /swagger-ui redirects to /swagger-ui/
        let res_redirect = client.get("/swagger-ui").dispatch().await;
        assert!(
            res_redirect.status() == Status::SeeOther || res_redirect.status() == Status::Found
        );
        assert_eq!(
            res_redirect.headers().get_one("Location"),
            Some("/swagger-ui/")
        );

        // 3. GET /api-docs/openapi.json returns HTTP 200 OK with valid JSON spec
        let res_json = client.get("/api-docs/openapi.json").dispatch().await;
        assert_eq!(res_json.status(), Status::Ok);
        let body_json = res_json.into_string().await.unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&body_json).expect("openapi.json must be valid JSON");
        assert_eq!(parsed["info"]["title"], "SPA Sax Backend API");
    }

    #[tokio::test]
    async fn test_swagger_runtime_mounting_production() {
        use rocket::http::Status;
        use rocket::local::asynchronous::Client;
        use spa_sax_backend::bootstrap::mount_swagger;

        let rocket = mount_swagger(rocket::build(), "production");
        let client = Client::untracked(rocket)
            .await
            .expect("Failed to build Rocket client");

        // 1. GET /swagger-ui/ returns 404 in production
        let res_ui = client.get("/swagger-ui/").dispatch().await;
        assert_eq!(res_ui.status(), Status::NotFound);

        // 2. GET /api-docs/openapi.json returns 404 in production
        let res_json = client.get("/api-docs/openapi.json").dispatch().await;
        assert_eq!(res_json.status(), Status::NotFound);
    }
}
