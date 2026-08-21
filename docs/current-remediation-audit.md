# Current Remediation Audit - spa-sax-backend

This document details the issues identified during the initial baseline inspection (Phase 0).

## 1. Quality & Infrastructure

### [BLOCKER] Docker Database Connectivity
- **Severity:** BLOCKER
- **Affected Files:** `docker-compose.yml`, `.env.example`
- **Current Behaviour:** `.env.example` uses `localhost` for `DATABASE_URL`. Docker container tries to connect to `localhost`, which fails inside the container.
- **Expected Behaviour:** API container should use `postgres` as hostname to connect to the database container.
- **Planned Fix:** Explicitly set `DATABASE_URL` in `docker-compose.yml` environment or use a separate env for Docker.
- **Acceptance Test:** `docker compose up --build` starts API and connects to DB.

### [CRITICAL] Missing Database Migration Strategy
- **Severity:** CRITICAL
- **Affected Files:** `Dockerfile`, `docker-compose.yml`
- **Current Behaviour:** API starts without ensuring migrations are run. Dockerfile copies migrations but doesn't run them.
- **Expected Behaviour:** Migrations should run before API starts.
- **Planned Fix:** Create a migration binary (`src/bin/migrate.rs`) and a one-shot `migrate` service in Docker Compose.
- **Acceptance Test:** `docker compose up` results in a fully migrated database.

### [MEDIUM] Production Seed Data in Migrations
- **Severity:** MEDIUM
- **Affected Files:** `migrations/0003_seed_mock_data.sql`
- **Current Behaviour:** Seed data is mixed with migrations.
- **Expected Behaviour:** Migrations should only contain schema. Seed data should be separate.
- **Planned Fix:** Move seeds to `scripts/seed-development.sql`.
- **Acceptance Test:** Clean migration doesn't create mock users.

---

## 2. Security

### [CRITICAL] Insecure Refresh Token Generation & Storage
- **Severity:** CRITICAL
- **Affected Files:** `src/infrastructure/auth/mod.rs` (needs inspection)
- **Current Behaviour:** Prompt indicates tokens might be UUID v4 and not securely hashed.
- **Expected Behaviour:** 256-bit entropy, hashed storage, transactional rotation with reuse detection.
- **Planned Fix:** Implement cryptographically secure token generation and transactional rotation.
- **Acceptance Test:** Unit tests for entropy and rotation logic.

### [CRITICAL] Fragile JWT Claims & State Verification
- **Severity:** CRITICAL
- **Affected Files:** `src/infrastructure/auth/mod.rs`
- **Current Behaviour:** Doesn't track `auth_version`. Relies on JWT role without verifying user state in DB.
- **Expected Behaviour:** Token contains `auth_version`, verified against DB on every request.
- **Planned Fix:** Add `auth_version` to `users` and JWT claims. Implement strict verification.
- **Acceptance Test:** Changing password or deactivating user invalidates existing JWTs.

### [HIGH] Case-Insensitive Email Uniqueness
- **Severity:** HIGH
- **Affected Files:** `migrations/0001_initial_schema.sql`, `src/bin/create_admin.rs`
- **Current Behaviour:** Uses `LOWER(email)` in indexes and manual normalization.
- **Expected Behaviour:** Use PostgreSQL `CITEXT` for robust case-insensitivity.
- **Planned Fix:** Migration to add `CITEXT` and update `users` table.
- **Acceptance Test:** Attempting to register `USER@example.com` when `user@example.com` exists returns `DUPLICATE_EMAIL`.

### [HIGH] Insecure Admin CLI
- **Severity:** HIGH
- **Affected Files:** `src/bin/create_admin.rs`
- **Current Behaviour:** Password input is visible on screen. No proper reporting of row insertion.
- **Expected Behaviour:** Hidden password input using `rpassword`.
- **Planned Fix:** Refactor `create_admin.rs`.
- **Acceptance Test:** Running the CLI doesn't echo password.

### [MEDIUM] LocalStorage Path Security
- **Severity:** MEDIUM
- **Affected Files:** `src/infrastructure/storage/mod.rs` (needs inspection)
- **Current Behaviour:** Basic path sanitization.
- **Expected Behaviour:** Strict validation against directory traversal.
- **Planned Fix:** Implement strict component-based path validation.
- **Acceptance Test:** Unit tests with `../` attempts fail.

---

## 3. Architecture & Functional

### [CRITICAL] Disconnected Modules & Placeholders
- **Severity:** CRITICAL
- **Affected Files:** `src/domain/pages/mod.rs`, `src/api/responders/mod.rs`, etc.
- **Current Behaviour:** Placeholder files with no real logic.
- **Expected Behaviour:** Full implementation of CMS and public API.
- **Planned Fix:** Implement missing services and connect to routes.
- **Acceptance Test:** Routes return real data from DB.

### [HIGH] Untyped Public API
- **Severity:** HIGH
- **Affected Files:** `src/api/routes/public.rs`, `src/application/dto/mod.rs`
- **Current Behaviour:** Uses `serde_json::Value` for responses.
- **Expected Behaviour:** Concrete typed DTOs.
- **Planned Fix:** Define and use proper DTOs.
- **Acceptance Test:** API response matches expected schema without generic JSON objects.

### [MEDIUM] Missing CORS & Request IDs
- **Severity:** MEDIUM
- **Affected Files:** `src/bootstrap/mod.rs`
- **Current Behaviour:** CORS may be configured but not fully applied. No request ID tracking.
- **Expected Behaviour:** Proper CORS fairing and X-Request-ID propagation.
- **Planned Fix:** Implement Rocket fairings for CORS and Request ID.
- **Acceptance Test:** Response headers contain `Access-Control-Allow-Origin` and `X-Request-ID`.
