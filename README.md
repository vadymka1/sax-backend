# SPA Sax Backend API - Production-Ready MVP

Backend API built with **Rust stable 2021**, **Rocket 0.5.1**, **PostgreSQL 17**, **SQLx**, **Argon2id**, **JWT auth**, and **Storage Abstraction**.

---

## 1. Scope & Architecture

Layered clean architecture:
`HTTP Route` ➔ `Request Guard / Validation` ➔ `Service` ➔ `Repository / DB` ➔ `DTO Response`

- **Authentication & RBAC**: `super_admin` & `admin` roles, Argon2id password hashing, short-lived Access JWT & 256-bit Refresh Token rotation.
- **Content Blocks**: Home page content blocks (`text`, `text_image`, `text_youtube`, `text_video`), transactional reordering, and soft delete.
- **Media Management**: Local file uploads (`image/jpeg`, `image/png`, `image/webp`, `video/mp4`, `video/webm`), streaming size validation, path sanitization (`Component::Normal`), and YouTube URL metadata parsing.
- **Public API**: Single aggregated `GET /api/v1/public/page` query returning home page blocks and media.

---

## 2. Environment Setup

Copy `.env.example` to `.env`:
```bash
cp .env.example .env
```

To run standalone locally without Docker, use `.env.local.example`:
```bash
cp .env.local.example .env
```

---

## 3. Docker Startup & Health Checks

Build and start containers:
```bash
make docker-up
```

Verify service status:
```bash
curl -f http://localhost:8000/health/live
curl -f http://localhost:8000/health/ready
curl -f http://localhost:8000/api/v1/public/page
```

---

## 4. Migrations & Bootstrap Super Admin

Migrations run automatically inside Docker via the `migrate` container before starting `api`.

For manual execution:
```bash
cargo run --bin migrate
```

To create your initial Super Admin user securely:
```bash
cargo run --bin create_admin
```

---

## 5. API Endpoints Overview

| Method | Endpoint | Description | Auth Required |
|---|---|---|:---:|
| `GET` | `/health` | Liveness & Readiness checks | No |
| `POST` | `/api/v1/auth/login` | Super Admin / Admin login | No |
| `POST` | `/api/v1/auth/refresh` | Refresh Access Token (256-bit rotation) | No |
| `POST` | `/api/v1/auth/logout` | Revoke Refresh Token | No |
| `GET` | `/api/v1/auth/me` | Current user profile | Bearer JWT |
| `GET` | `/api/v1/admin/users` | List admin users | Bearer JWT (super_admin) |
| `POST` | `/api/v1/admin/users` | Create new admin user (`admin` or `super_admin` role) | Bearer JWT (super_admin) |
| `PATCH` | `/api/v1/admin/users/:id` | Update user (`display_name`, `role`, `is_active`) | Bearer JWT (super_admin) |
| `POST` | `/api/v1/admin/users/:id/activate` | Activate admin account | Bearer JWT (super_admin) |
| `POST` | `/api/v1/admin/users/:id/deactivate` | Deactivate admin account (self-deactivation guarded) | Bearer JWT (super_admin) |
| `GET` | `/api/v1/admin/spa-sections` | List dynamic SPA sections | Bearer JWT |
| `POST` | `/api/v1/admin/spa-sections` | Create dynamic SPA section | Bearer JWT |
| `GET` | `/api/v1/admin/spa-sections/:id` | Get dynamic SPA section | Bearer JWT |
| `PATCH` | `/api/v1/admin/spa-sections/:id` | Update dynamic SPA section | Bearer JWT |
| `DELETE` | `/api/v1/admin/spa-sections/:id` | Soft delete empty SPA section | Bearer JWT |
| `POST` | `/api/v1/admin/spa-sections/reorder` | Reorder dynamic SPA sections | Bearer JWT |
| `GET` | `/api/v1/admin/content-blocks` | List home page blocks | Bearer JWT |
| `POST` | `/api/v1/admin/content-blocks` | Create content block | Bearer JWT |
| `GET` | `/api/v1/admin/content-blocks/:id` | Get content block by ID | Bearer JWT |
| `PATCH` | `/api/v1/admin/content-blocks/:id` | Update content block | Bearer JWT |
| `DELETE` | `/api/v1/admin/content-blocks/:id` | Soft delete content block | Bearer JWT |
| `POST` | `/api/v1/admin/spa-sections/:spa_section_id/content-blocks/reorder` | Per-section transactional block reorder | Bearer JWT |
| `POST` | `/api/v1/admin/media/upload` | Upload image / video file | Bearer JWT |
| `POST` | `/api/v1/admin/media/youtube` | Register YouTube video URL | Bearer JWT |
| `GET` | `/api/v1/admin/media` | List media assets | Bearer JWT |
| `GET` | `/api/v1/admin/media/:id` | Get media asset details | Bearer JWT |
| `DELETE` | `/api/v1/admin/media/:id` | Delete unused media asset | Bearer JWT |
| `GET` | `/api/v1/public/page` | Grouped public home page response (sections & blocks) | No |

### Public SPA Response Example (`GET /api/v1/public/page`)
```json
{
  "data": {
    "page": {
      "slug": "home",
      "title": "Home"
    },
    "sections": [
      {
        "key": "about-us",
        "title": "About Us",
        "navigation_label": "About Us",
        "sort_order": 10,
        "blocks": []
      }
    ]
  }
}
```

---

## OpenAPI / Swagger

OpenAPI 3.0 documentation and Swagger UI are enabled automatically when running in the development environment (`APP_ENV=development`).

### Environment Configuration
```env
APP_ENV=development
```

### Canonical URLs
- **Swagger UI**: [http://localhost:8000/swagger-ui/](http://localhost:8000/swagger-ui/) (Requests to `/swagger-ui` automatically redirect to `/swagger-ui/`)
- **OpenAPI Spec (JSON)**: [http://localhost:8000/api-docs/openapi.json](http://localhost:8000/api-docs/openapi.json)

### Authorization Workflow in Swagger UI
1. Open [http://localhost:8000/swagger-ui/](http://localhost:8000/swagger-ui/) in your browser.
2. Execute `POST /api/v1/auth/login` with your credentials (`email`, `password`).
3. Copy the returned `access_token` string from the response JSON body.
4. Click the **Authorize** button at the top right of the Swagger UI interface.
5. In the `bearer_auth` modal, paste the raw access token (Swagger UI automatically prepends `Bearer `).
6. Click **Authorize** and execute protected `/api/v1/admin/*` endpoints directly from Swagger UI.

### Production Environment Policy
When `APP_ENV != development` (e.g. `APP_ENV=production`), Swagger UI and OpenAPI JSON routes are **not mounted** for security.

---

## 6. Testing & Quality Gate

Run full verification suite:
```bash
make check
```
Or run individual targets:
```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
docker compose config
```

---

## 7. Known MVP Limitations
1. Single-page Focus: Content block CRUD is optimized for the `home` page.
2. Local Storage Provider: Production S3/Cloud Storage provider abstraction ready for Phase 2.
3. Media Attachments: Content blocks support single primary media cover attachment. Multiple gallery media items can be enabled in Phase 2.
