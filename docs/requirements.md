# Functional and Non-Functional Requirements

## 1. Executive Summary
This document specifies the backend system requirements for a personal/business showcase website API (inspired by high-end portfolio sites like Osanna Sax). The system serves two primary APIs:
- **Private Admin API**: Authenticated content and user management panel backend.
- **Public Website API**: High-performance, read-only aggregated API returning ready-to-render public site content.

## 2. Functional Requirements

### 2.1 Authentication & Authorization
- **FR-AUTH-1**: User login with Email & Argon2id hashed password returning short-lived Access JWT & Refresh JWT.
- **FR-AUTH-2**: Refresh token rotation storing SHA-256 hashed refresh tokens in database with revocation support.
- **FR-AUTH-3**: User logout revoking refresh tokens.
- **FR-AUTH-4**: Role-Based Access Control (RBAC) with roles `super_admin`, `admin`, and `editor`.
- **FR-AUTH-5**: Brute-force protection & generic error messaging for invalid credentials.

### 2.2 User Management (Super Admin & Admin)
- **FR-USER-1**: Paginated, filterable, sortable list of system users.
- **FR-USER-2**: User creation, updates, activation/deactivation, role promotion/demotion, and password resets.
- **FR-USER-3**: Deactivation safety checks (cannot deactivate self or last `super_admin`).

### 2.3 CMS (Pages, Sections, Content Items)
- **FR-CMS-1**: Single/multi-page architecture starting with `home` page.
- **FR-CMS-2**: Section ordering, visibility toggling (`draft`, `published`, `archived`), type tags (`hero`, `about`, `services`, `gallery`, `video`, `contact`, `social_links`, `cta`, `footer`, `custom`).
- **FR-CMS-3**: Section reordering executed within an isolated database transaction.
- **FR-CMS-4**: Content items belonging to sections with ordering and visibility support.

### 2.4 Media & Storage Asset Management
- **FR-MEDIA-1**: Storage provider abstraction (`StorageProvider` trait) initially backed by local file storage with clean UUID filenames.
- **FR-MEDIA-2**: Upload validation for MIME types (`image/jpeg`, `image/png`, `image/webp`, `video/mp4`, `video/webm`) and max file size limits.
- **FR-MEDIA-3**: YouTube URL parsing & validation into canonical embed URLs and thumbnail generation without accepting raw HTML iframes.
- **FR-MEDIA-4**: Linking media assets to sections with usage types (`background`, `cover`, `gallery`, `thumbnail`, `inline`, `video`, `og_image`).

### 2.5 Site Settings & Social Links
- **FR-SET-1**: Key-value pair configuration for global site settings (`site_name`, `contact_email`, `contact_phone`, `booking_url`, etc.).
- **FR-SET-2**: Social link ordering and visibility control.

### 2.6 Audit Logging
- **FR-AUDIT-1**: Logging administrative mutation events with actor ID, entity reference, IP, User-Agent, and sanitized old/new JSON payloads. Sensitive fields (passwords, tokens) must never be logged.

### 2.7 Public Aggregated API
- **FR-PUB-1**: Aggregate endpoint `GET /api/v1/public/page` returning site metadata, SEO tags, active published sections, sorted content items, valid media URLs, and social links.
- **FR-PUB-2**: Exclusion of internal metadata (`created_by`, password hashes, drafts, archived content).
- **FR-PUB-3**: ETag conditional requests (`If-None-Match` returning `304 Not Modified`).

## 3. Non-Functional Requirements

### 3.1 Security & Compliance
- Security headers (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`, `Content-Security-Policy`).
- Configurable CORS allowed origins.
- Prepared SQL statements (SQLx) preventing SQL injection.
- Absolute protection against path traversal on upload/download routes.

### 3.2 Reliability & Performance
- Sub-50ms public API response times (accelerated by ETag caching).
- Clean shutdown and connection pool health monitoring.

### 3.3 Maintainability & Code Quality
- Rust 2021 edition, strict clippy compliance without unhandled errors (`unwrap()` / `expect()` prohibited in request handlers).
