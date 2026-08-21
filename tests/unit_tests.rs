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
}
