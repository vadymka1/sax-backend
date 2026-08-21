use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, Method, Status};
use rocket::{Data, Request, Response};
use uuid::Uuid;

use crate::api::catchers::get_or_create_request_id;
use crate::config::AppConfig;

pub struct CorsFairing;

#[rocket::async_trait]
impl Fairing for CorsFairing {
    fn info(&self) -> Info {
        Info {
            name: "CORS & Request ID Fairing",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        // Validate or generate request_id
        let client_req_id = req.headers().get_one("X-Request-ID");
        let safe_req_id = match client_req_id {
            Some(id)
                if id.len() <= 64
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
            {
                id.to_string()
            }
            _ => Uuid::new_v4().to_string(),
        };

        // Attach local state request_id to Request
        req.local_cache(|| safe_req_id);
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let req_id = get_or_create_request_id(req);
        res.set_header(Header::new("X-Request-ID", req_id));

        let origin = match req.headers().get_one("Origin") {
            Some(o) => o.trim(),
            None => return,
        };

        let config = match req.rocket().state::<AppConfig>() {
            Some(c) => c,
            None => return,
        };

        let is_allowed = config.cors_allowed_origins.iter().any(|allowed| {
            let trimmed = allowed.trim();
            !trimmed.is_empty() && trimmed == origin
        });

        if is_allowed {
            res.set_header(Header::new(
                "Access-Control-Allow-Origin",
                origin.to_string(),
            ));
            res.set_header(Header::new("Vary", "Origin"));
            res.set_header(Header::new(
                "Access-Control-Allow-Methods",
                "GET, POST, PATCH, DELETE, OPTIONS",
            ));
            res.set_header(Header::new(
                "Access-Control-Allow-Headers",
                "Authorization, Content-Type, X-Request-ID",
            ));
            res.set_header(Header::new(
                "Access-Control-Expose-Headers",
                "X-Request-ID, ETag",
            ));
            res.set_header(Header::new("Access-Control-Max-Age", "86400"));
        } else if req.method() == Method::Options {
            // Option B: Return 403 Forbidden for preflight requests from unknown origins
            res.set_status(Status::Forbidden);
            return;
        }

        // Handle OPTIONS preflight status 204 No Content for allowed origins
        if req.method() == Method::Options && is_allowed {
            res.set_status(Status::NoContent);
        }
    }
}
