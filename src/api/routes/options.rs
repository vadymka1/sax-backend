use rocket::http::Status;
use std::path::PathBuf;

/// Catch-all preflight OPTIONS handler for application routes with paths.
///
/// Handles CORS preflight requests so Rocket recognizes matching routes instead of
/// emitting 404 routing errors. Origin validation and CORS headers are handled
/// dynamically in `CorsFairing`.
#[rocket::options("/<_path..>")]
pub fn preflight_options(_path: PathBuf) -> Status {
    Status::NoContent
}

/// Catch-all preflight OPTIONS handler for application root mount.
#[rocket::options("/")]
pub fn preflight_options_root() -> Status {
    Status::NoContent
}
