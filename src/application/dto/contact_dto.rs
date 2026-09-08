use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ContactRequest {
    pub name: String,
    pub email: String,
    pub subject: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ContactResponse {
    pub status: String,
    pub message: String,
}
