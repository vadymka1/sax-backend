# MVP Fix and Implementation Plan

## Stabilization & Feature Implementation Plan

1. **Phase 1: Baseline Stabilization (Current)**
   - Complete clean baseline code check across all toolchain commands.
   - Enforce repository hygiene (`.gitignore`, cleanup temporary files).
   - Truthful README documentation update.

2. **Phase 2: Authentication & User Management Extensions**
   - Implement missing auth endpoints: `/auth/logout`, `/auth/me`, `/auth/change-password`.
   - Implement full user management CRUD (`PATCH /admin/users/{id}`, `POST /admin/users/{id}/activate`, `POST /admin/users/{id}/deactivate`, `POST /admin/users/{id}/reset-password`).
   - Add filtering, search, and sorting logic to user list.

3. **Phase 3: CMS & Content Block API**
   - Complete Section CRUD (`GET`, `POST`, `PATCH`, `DELETE`, `publish`, `unpublish`).
   - Implement Content Item CRUD and reordering (`/admin/sections/{id}/items`).

4. **Phase 4: Media Uploads & Storage Provider Integration**
   - Implement Rocket `multipart/form-data` upload endpoint (`POST /admin/media/upload`).
   - Enforce MIME type validation and maximum file size limits from configuration.

5. **Phase 5: Integration Testing & Production Verification**
   - Create isolated test database integration tests in `tests/integration/`.
   - Verify Docker environment startup and end-to-end API flows.
