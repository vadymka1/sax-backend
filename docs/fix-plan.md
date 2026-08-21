# Fix Plan - spa-sax-backend

This document outlines the phased approach to fixing and completing the backend.

## Phase 0: Baseline & Hygiene (Completed)
- [x] Run baseline checks (`fmt`, `check`, `clippy`, `test`).
- [x] Setup `.env` and `.gitignore`.
- [x] Clean up repository junk (`.DS_Store`, `__MACOSX`).
- [x] Identify disconnected/placeholder modules.
- [x] Create `current-remediation-audit.md`.

## Phase 1: Docker & Migrations
- [ ] Fix `DATABASE_URL` for Docker.
- [ ] Create `src/bin/migrate.rs` for transactional migrations.
- [ ] Update `docker-compose.yml` with `migrate` service and `postgres` hostname.
- [ ] Separate seed data: move `0003_seed_mock_data.sql` content to `scripts/seed-development.sql`.
- [ ] Implement `create_admin` CLI with `rpassword` and proper normalization.
- [ ] Fix email uniqueness: Migration for `CITEXT`.

## Phase 2: Security Foundation
- [ ] Implement Request ID fairing.
- [ ] Implement CORS fairing with origin allowlist.
- [ ] Secure Refresh Token generation (256-bit entropy, hashed storage).
- [ ] Transactional Refresh Token rotation with reuse detection.
- [ ] JWT `auth_version` support and user-state verification.
- [ ] Storage path security (strict component validation).
- [ ] Rate limiting for sensitive endpoints (login, refresh, upload).

## Phase 3: Authentication & Users
- [ ] Refactor `AuthService` and `UserService`.
- [ ] Implement full auth API (login, refresh, logout, logout-all, /me, change-password).
- [ ] Implement central permissions system (`Permission` enum).
- [ ] Implement Admin User Management API (list, create, update, activate/deactivate, reset-password).
- [ ] Integrate Audit Logging for all mutations.

## Phase 4: CMS (Content Management System)
- [ ] Implement Page Management (CRUD, publish/unpublish).
- [ ] Implement Section Management (CRUD, publish/unpublish, reorder).
- [ ] Implement Content Item Management (CRUD, reorder).
- [ ] Implement Site Settings & Social Links API.

## Phase 5: Media Management
- [ ] Implement Media Upload (multipart/form-data) with validation.
- [ ] Implement YouTube media integration (URL parsing with `url` crate).
- [ ] Media attachment/detachment policy.
- [ ] Database/File consistency policy.

## Phase 6: Public API
- [ ] Replace `serde_json::Value` with concrete DTOs.
- [ ] Implement aggregated public page API (optimized queries, memory grouping).
- [ ] Implement ETag & Cache-Control support.

## Phase 7: Delivery & Finalization
- [ ] Complete OpenAPI (utoipa) documentation for all routes.
- [ ] Fix/Update GitHub Actions CI.
- [ ] Update Dockerfile (multi-stage, non-root, minimal).
- [ ] Final README update.
