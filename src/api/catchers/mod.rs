use crate::shared::errors::{ApiErrorPayload, ApiErrorResponse};
use rocket::catch;
use rocket::request::Request;
use rocket::serde::json::Json;

pub fn get_or_create_request_id(req: &Request<'_>) -> String {
    req.local_cache(|| uuid::Uuid::new_v4().to_string()).clone()
}

#[catch(404)]
pub fn not_found(req: &Request<'_>) -> Json<ApiErrorResponse> {
    let req_id = get_or_create_request_id(req);
    Json(ApiErrorResponse {
        error: ApiErrorPayload {
            code: "RESOURCE_NOT_FOUND".to_string(),
            message: "The requested route or resource was not found".to_string(),
            details: None,
            request_id: req_id,
        },
    })
}

#[catch(500)]
pub fn internal_error(req: &Request<'_>) -> Json<ApiErrorResponse> {
    let req_id = get_or_create_request_id(req);
    Json(ApiErrorResponse {
        error: ApiErrorPayload {
            code: "INTERNAL_SERVER_ERROR".to_string(),
            message: "An unexpected internal server error occurred".to_string(),
            details: None,
            request_id: req_id,
        },
    })
}
