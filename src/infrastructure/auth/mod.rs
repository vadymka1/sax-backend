use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::users::Role;
use crate::shared::errors::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct PasswordService;

impl PasswordService {
    pub fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AppError::Internal(format!("Password hashing error: {}", e)))
    }

    pub fn verify_password(password: &str, hash_str: &str) -> bool {
        let parsed_hash = match PasswordHash::new(hash_str) {
            Ok(h) => h,
            Err(_) => return false,
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
}

pub struct TokenService;

impl TokenService {
    pub fn generate_access_token(
        user_id: Uuid,
        email: &str,
        role: Role,
        secret: &str,
        ttl_seconds: i64,
    ) -> AppResult<String> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.as_str().to_string(),
            exp: now + ttl_seconds,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("JWT token generation error: {}", e)))
    }

    pub fn decode_access_token(token: &str, secret: &str) -> AppResult<Claims> {
        let validation = Validation::default();
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .map_err(|_| AppError::TokenExpired)
    }

    pub fn generate_refresh_token() -> String {
        let mut bytes = [0u8; 32]; // 256 bits of cryptographically secure randomness
        OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    pub fn hash_refresh_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
