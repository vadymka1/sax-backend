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
| `POST` | `/api/v1/admin/content-blocks` | Create content block (multi-image carousel, typography) | Bearer JWT |
| `GET` | `/api/v1/admin/content-blocks/:id` | Get content block by ID | Bearer JWT |
| `PATCH` | `/api/v1/admin/content-blocks/:id` | Update content block | Bearer JWT |
| `DELETE` | `/api/v1/admin/content-blocks/:id` | Soft delete content block | Bearer JWT |
| `POST` | `/api/v1/admin/spa-sections/:spa_section_id/content-blocks/reorder` | Per-section transactional block reorder (duplicate-safe normalization) | Bearer JWT |
| `POST` | `/api/v1/admin/media/upload` | Upload image / video file | Bearer JWT |
| `POST` | `/api/v1/admin/media/youtube` | Register YouTube video URL | Bearer JWT |
| `GET` | `/api/v1/admin/media` | List media assets | Bearer JWT |
| `GET` | `/api/v1/admin/media/:id` | Get media asset details | Bearer JWT |
| `DELETE` | `/api/v1/admin/media/:id` | Delete unused media asset | Bearer JWT |
| `GET` | `/api/v1/public/page` | Grouped public home page response (canonical 4 sections & blocks) | No |
| `POST` | `/api/v1/public/contact` | Submit public contact form (persists to DB & optional SMTP dispatch) | No |

---

## 6. Features & Enhancements

### 1. Multi-Image Carousel for Content Blocks
- `text_image` content blocks support zero, one, or multiple images (`media_ids: [uuid1, uuid2, ...]`).
- Backward-compatible with single `media_id` on both create and patch.
- Deterministic media ordering (`sort_order ASC, created_at ASC, id ASC`).
- Enforces media type validation (only `image` type assets allowed for `text_image` blocks).
- Rejects duplicate media IDs within the same block (`uq_section_media_block_asset`).

### 2. Default Four Sections
The home page provides 4 canonical visible sections in fixed order:
1. **About Us** (`about-us`, sort order `10`)
2. **Gallery** (`gallery`, sort order `20`)
3. **Contact Us** (`contact-us`, sort order `30`)
4. **Testimonials** (`testimonials`, sort order `40`)

Migration 0011 uses idempotent upserts (`INSERT ... ON CONFLICT (page_id, section_key) DO UPDATE`) to guarantee that all four canonical sections exist even if individual sections were deleted or missed during earlier setup. Existing section UUIDs and content blocks are strictly preserved. Old default sections (`our-works`, `festivals`) are retained with `is_visible = false` to preserve existing user content blocks without deletion.

### 3. Public Contact Form & SMTP Delivery
- Unauthenticated endpoint `POST /api/v1/public/contact`.
- **Database First**: Every submission is validated and saved to PostgreSQL table `contact_messages` with status tracking (`pending`, `sent`, `failed`, `disabled`).
- **SMTP Isolation**: Notification delivery via SMTP uses `lettre` with TLS. SMTP network failures never fail the client HTTP request or roll back DB persistence.
- **Fail-Safe Secret Protection**: Passwords in SMTP error messages are redacted with `[REDACTED]` and truncated to 500 characters in `email_error`.
- **Startup Config Validation**: When `SMTP_ENABLED=true`, validates required host, port, sender, and recipient addresses at startup. When `SMTP_ENABLED=false`, zero network calls are attempted.

### 4. Content Block Typography
Content blocks persist typography tokens:
- `font_family`: `sans` (default), `serif`, `display`, `mono`
- `font_size`: `sm`, `md` (default), `lg`, `xl`, `2xl`

### 5. Robust Content Block Reordering
- Solves intermittent `VALIDATION_ERROR` during Move Up / Move Down or fast consecutive clicks.
- Deterministic sequence normalization (`(idx + 1) * 10`) regardless of duplicate input sort orders.
- Two-phase collision-free database updates.

### 6. Robust SPA Section Reordering
- Eliminates `VALIDATION_ERROR` on duplicate or intermediate requested sort orders during UI drag/swap operations.
- Incoming order values are treated as ordering hints; the backend deterministically canonicalizes to 10-step values (`10, 20, 30, ...`) using request index as tie-breaker.
- Two-phase database transaction ensuring zero collision errors under PostgreSQL check constraints (`sort_order >= 0`).

---

## 7. OpenAPI / Swagger

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

## 8. Testing & Quality Gate

Run full verification suite:
```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
APP_ENV=test DATABASE_URL=postgres://app:app_password@localhost:5432/app_test cargo test --test integration_tests -- --test-threads=1
cargo test --test unit_tests
cargo build --release
```

---

## 9. Known MVP Limitations
1. Single-page Focus: Content block CRUD is optimized for the `home` page.
2. Local Storage Provider: Production S3/Cloud Storage provider abstraction ready for Phase 2.
