# Security Architecture & Threat Model

## 1. Authentication & Token Lifecycle
- **Password Hashing**: Argon2id via `argon2` crate using recommended parameters (Memory 19MiB, 2 iterations, 1 parallelism degree).
- **Access Tokens**: Short-lived (15 minutes default) HMAC-SHA256 JWTs signed with `JWT_ACCESS_SECRET`. Contain `sub` (User UUID), `role`, `exp`, `iat`, `nbf`, `jti`.
- **Refresh Tokens**: Long-lived (30 days default) cryptographically random UUID strings. Stored in DB exclusively as SHA-256 hashes (`user_refresh_tokens`). On `/auth/refresh`, old token is marked `revoked_at` and a new token pair is issued (token rotation).

## 2. Authorization & RBAC Matrix

| Endpoint Group | `super_admin` | `admin` | `editor` | Public |
| :--- | :---: | :---: | :---: | :---: |
| `POST /api/v1/auth/login` | Yes | Yes | Yes | Yes |
| `POST /api/v1/auth/refresh` | Yes | Yes | Yes | Yes |
| `GET /api/v1/auth/me` | Yes | Yes | Yes | No |
| `GET /api/v1/admin/users` | Yes | Yes | No | No |
| `POST /api/v1/admin/users` | Yes | Yes (editor only) | No | No |
| `POST /api/v1/admin/users/{id}/deactivate` | Yes | Yes (editor only) | No | No |
| `PATCH /api/v1/admin/users/{id}` (Role Change) | Yes | No | No | No |
| `GET/POST/PATCH/DELETE /api/v1/admin/pages/*` | Yes | Yes | Yes | No |
| `GET/POST/PATCH/DELETE /api/v1/admin/sections/*` | Yes | Yes | Yes | No |
| `POST /api/v1/admin/media/upload` | Yes | Yes | Yes | No |
| `GET /api/v1/admin/audit-logs` | Yes | Yes | No | No |
| `GET /api/v1/public/page` | Yes | Yes | Yes | Yes |

## 3. Media Upload & Input Sanitization
- Storage filenames are auto-generated non-predictable UUID v4 names preserving validated extension (`<uuid>.<ext>`). Original filenames are stored only in metadata columns after HTML encoding.
- Direct path traversal attacks (`../`, `..\`) are impossible as paths are generated strictly via systemUUID + fixed storage roots (`uploads/images/YYYY/MM`).
- YouTube URL parser restricts allowed protocols to `https://` and strict host matching (`youtube.com`, `youtu.be`).

## 4. HTTP Headers & CORS
- Security Headers fairing sets:
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY`
  - `Referrer-Policy: strict-origin-when-cross-origin`
  - `X-XSS-Protection: 1; mode=block`
- CORS fairing matches request origin against comma-separated `CORS_ALLOWED_ORIGINS` environment variable. Preflight OPTIONS requests are handled with maximum 3600s cache age.
