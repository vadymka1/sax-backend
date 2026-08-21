use rocket::http::Status;
use rocket::response::{self, Responder, Response};
use rocket::serde::json::Json;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct ApiErrorDetails {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ApiErrorDetails>>,
    pub request_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorResponse {
    pub error: ApiErrorPayload,
}

#[derive(Debug)]
pub enum AppError {
    ValidationError(Vec<ApiErrorDetails>),
    Unauthorized,
    Forbidden,
    InvalidCredentials,
    TokenExpired,
    TokenRevoked,
    UserNotFound,
    UserInactive,
    NotFound(String),
    ResourceNotFound(String),
    ResourceConflict(String),
    DuplicateEmail,
    InvalidMediaType,
    FileTooLarge,
    InvalidYoutubeUrl,
    DatabaseError(String),
    Internal(String),
    RateLimitExceeded,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::ValidationError(_) => "VALIDATION_ERROR",
            AppError::Unauthorized => "UNAUTHORIZED",
            AppError::Forbidden => "FORBIDDEN",
            AppError::InvalidCredentials => "INVALID_CREDENTIALS",
            AppError::TokenExpired => "TOKEN_EXPIRED",
            AppError::TokenRevoked => "TOKEN_REVOKED",
            AppError::UserNotFound => "USER_NOT_FOUND",
            AppError::UserInactive => "USER_INACTIVE",
            AppError::NotFound(_) | AppError::ResourceNotFound(_) => "RESOURCE_NOT_FOUND",
            AppError::ResourceConflict(_) => "RESOURCE_CONFLICT",
            AppError::DuplicateEmail => "DUPLICATE_EMAIL",
            AppError::InvalidMediaType => "INVALID_MEDIA_TYPE",
            AppError::FileTooLarge => "FILE_TOO_LARGE",
            AppError::InvalidYoutubeUrl => "INVALID_YOUTUBE_URL",
            AppError::DatabaseError(_) => "DATABASE_ERROR",
            AppError::Internal(_) => "INTERNAL_SERVER_ERROR",
            AppError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
        }
    }

    pub fn status(&self) -> Status {
        match self {
            AppError::ValidationError(_) => Status::UnprocessableEntity,
            AppError::Unauthorized | AppError::TokenExpired | AppError::TokenRevoked => {
                Status::Unauthorized
            }
            AppError::InvalidCredentials => Status::Unauthorized,
            AppError::Forbidden => Status::Forbidden,
            AppError::UserNotFound | AppError::NotFound(_) | AppError::ResourceNotFound(_) => {
                Status::NotFound
            }
            AppError::UserInactive => Status::Forbidden,
            AppError::ResourceConflict(_) | AppError::DuplicateEmail => Status::Conflict,
            AppError::InvalidMediaType | AppError::FileTooLarge | AppError::InvalidYoutubeUrl => {
                Status::BadRequest
            }
            AppError::RateLimitExceeded => Status::TooManyRequests,
            AppError::DatabaseError(_) | AppError::Internal(_) => Status::InternalServerError,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AppError::ValidationError(_) => "Request validation failed".to_string(),
            AppError::Unauthorized => "Authentication required".to_string(),
            AppError::Forbidden => "Permission denied".to_string(),
            AppError::InvalidCredentials => "Invalid email or password".to_string(),
            AppError::TokenExpired => "Access token expired".to_string(),
            AppError::TokenRevoked => "Refresh token revoked".to_string(),
            AppError::UserNotFound => "User not found".to_string(),
            AppError::UserInactive => "User account is inactive".to_string(),
            AppError::NotFound(msg) | AppError::ResourceNotFound(msg) => msg.clone(),
            AppError::ResourceConflict(msg) => msg.clone(),
            AppError::DuplicateEmail => "Email address is already in use".to_string(),
            AppError::InvalidMediaType => "Unsupported media type".to_string(),
            AppError::FileTooLarge => "Uploaded file exceeds maximum size limit".to_string(),
            AppError::InvalidYoutubeUrl => "Invalid YouTube URL format".to_string(),
            AppError::DatabaseError(_) => "A database error occurred".to_string(),
            AppError::Internal(_) => "An internal server error occurred".to_string(),
            AppError::RateLimitExceeded => "Too many requests".to_string(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for AppError {}

impl<'r> Responder<'r, 'static> for AppError {
    fn respond_to(self, req: &'r rocket::Request<'_>) -> response::Result<'static> {
        let status = self.status();
        let request_id = req.local_cache(|| uuid::Uuid::new_v4().to_string()).clone();

        let details = match &self {
            AppError::ValidationError(d) => Some(
                d.iter()
                    .map(|item| ApiErrorDetails {
                        field: item.field.clone(),
                        message: item.message.clone(),
                    })
                    .collect(),
            ),
            _ => None,
        };

        let payload = ApiErrorResponse {
            error: ApiErrorPayload {
                code: self.code().to_string(),
                message: self.message(),
                details,
                request_id,
            },
        };

        Response::build_from(Json(payload).respond_to(req)?)
            .status(status)
            .ok()
    }
}
