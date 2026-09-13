use rocket::serde::json::Json;
use rocket::{get, post, State};
use uuid::Uuid;

use crate::api::guards::AuthenticatedUser;
use crate::application::dto::{AdminContactMessageDto, ContactRequest, ContactResponse};
use crate::application::services::ContactService;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};
use crate::shared::pagination::SingleResponse;

/// Submit Contact Us Form
///
/// Public endpoint to submit an inquiry through the Contact Us form.
/// Validates input, securely saves the inquiry to PostgreSQL, and triggers an email notification via SMTP if configured.
#[utoipa::path(
    post,
    path = "/api/v1/public/contact",
    tag = "Public",
    request_body = ContactRequest,
    responses(
        (status = 200, description = "Contact form submitted successfully", body = ContactResponse),
        (status = 422, description = "Validation error", body = ApiErrorResponse)
    )
)]
#[post("/public/contact", data = "<req>")]
pub async fn submit_contact_form(
    service: &State<ContactService>,
    req: Json<ContactRequest>,
) -> AppResult<Json<ContactResponse>> {
    let res = service.submit_contact(req.into_inner()).await?;
    Ok(Json(res))
}

/// List contact messages for admin inbox
///
/// Returns all contact inquiries ordered newest first. Supports optional status filter: all, read, unread. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/contact-messages",
    tag = "Contact Messages",
    security(("bearer_auth" = [])),
    params(
        ("status" = Option<String>, Query, description = "Optional status filter: all, read, unread")
    ),
    responses(
        (status = 200, description = "List of contact messages", body = SingleResponse<Vec<AdminContactMessageDto>>),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[get("/admin/contact-messages?<status>")]
pub async fn list_contact_messages(
    auth: AuthenticatedUser,
    service: &State<ContactService>,
    status: Option<String>,
) -> AppResult<Json<SingleResponse<Vec<AdminContactMessageDto>>>> {
    let messages = service.list_messages(&auth, status.as_deref()).await?;
    Ok(Json(SingleResponse { data: messages }))
}

/// Get contact message by ID
///
/// Returns full details of a contact message without changing its read/unread status. Requires authenticated super_admin or admin.
#[utoipa::path(
    get,
    path = "/api/v1/admin/contact-messages/{id}",
    tag = "Contact Messages",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "Contact message UUID identifier")
    ),
    responses(
        (status = 200, description = "Contact message details", body = SingleResponse<AdminContactMessageDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Contact message not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[get("/admin/contact-messages/<id_str>")]
pub async fn get_contact_message(
    auth: AuthenticatedUser,
    id_str: &str,
    service: &State<ContactService>,
) -> AppResult<Json<SingleResponse<AdminContactMessageDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid contact message ID".to_string(),
        }])
    })?;

    let message = service.get_message(&auth, id).await?;
    Ok(Json(SingleResponse { data: message }))
}

/// Mark contact message as read
///
/// Idempotently sets is_read = true and read_at = now() (or preserves existing read_at). Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/contact-messages/{id}/read",
    tag = "Contact Messages",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "Contact message UUID identifier")
    ),
    responses(
        (status = 200, description = "Contact message marked as read", body = SingleResponse<AdminContactMessageDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Contact message not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[post("/admin/contact-messages/<id_str>/read")]
pub async fn mark_contact_message_read(
    auth: AuthenticatedUser,
    id_str: &str,
    service: &State<ContactService>,
) -> AppResult<Json<SingleResponse<AdminContactMessageDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid contact message ID".to_string(),
        }])
    })?;

    let message = service.mark_as_read(&auth, id).await?;
    Ok(Json(SingleResponse { data: message }))
}

/// Mark contact message as unread
///
/// Idempotently sets is_read = false and read_at = NULL. Requires authenticated super_admin or admin.
#[utoipa::path(
    post,
    path = "/api/v1/admin/contact-messages/{id}/unread",
    tag = "Contact Messages",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "Contact message UUID identifier")
    ),
    responses(
        (status = 200, description = "Contact message marked as unread", body = SingleResponse<AdminContactMessageDto>),
        (status = 422, description = "Invalid UUID path parameter", body = ApiErrorResponse),
        (status = 401, description = "Missing or invalid Bearer access token", body = ApiErrorResponse),
        (status = 403, description = "Forbidden - Requires super_admin or admin role", body = ApiErrorResponse),
        (status = 404, description = "Contact message not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse)
    )
)]
#[post("/admin/contact-messages/<id_str>/unread")]
pub async fn mark_contact_message_unread(
    auth: AuthenticatedUser,
    id_str: &str,
    service: &State<ContactService>,
) -> AppResult<Json<SingleResponse<AdminContactMessageDto>>> {
    let id = Uuid::parse_str(id_str).map_err(|_| {
        AppError::ValidationError(vec![ApiErrorDetails {
            field: "id".to_string(),
            message: "Invalid contact message ID".to_string(),
        }])
    })?;

    let message = service.mark_as_unread(&auth, id).await?;
    Ok(Json(SingleResponse { data: message }))
}
