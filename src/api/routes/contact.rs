use rocket::serde::json::Json;
use rocket::{post, State};

use crate::application::dto::{ContactRequest, ContactResponse};
use crate::application::services::ContactService;
use crate::shared::errors::AppResult;

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
        (status = 422, description = "Validation error", body = crate::shared::errors::ApiErrorResponse)
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
