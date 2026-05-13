---
title: Security
description: Security model, defaults, and hardening guide for production deployments.
---

## Passwords

All passwords are hashed with **Argon2id** before storage:

- Parameters: 19 MB memory, 2 iterations, 1 thread (OWASP recommended minimum)
- Plaintext is never stored, logged, or returned in any response
- Resistant to GPU and ASIC brute-force attacks

**Password history** — when `history_size > 0` in a tenant's password policy, the last N hashes are checked on every password change (reset flow and admin-forced change). Reusing a recent password returns a 400 error. Each accepted hash is recorded in `password_history` (RLS-isolated per tenant). Setting `history_size = 0` disables the check.

## Account lockout

After **5 consecutive failed login attempts**, the account is locked for **15 minutes**.

- Lockout is per-user, per-tenant — one tenant's lockouts don't affect another
- Both thresholds are configurable per-tenant via Settings
- Stale attempt records are purged every 6 hours by the background cleanup task

## Token security

| Token | Details |
|-------|---------|
| Access token | HS256 JWT, 15 min default, JTI tracked in DB |
| Refresh token | Opaque, stored as hash, rotated on every use |
| id\_token | RS256, issued only on `authorization_code` flow |

- **JTI blocklist** — replayed access tokens are rejected at introspection even before expiry
- **Refresh token rotation** — a stolen refresh token used by an attacker invalidates the legitimate user's session immediately (detect-and-revoke)
- **Revocation** — `/auth/revoke` and `/oauth/revoke` propagate immediately; no cache delay

## Transport security

In production (`ENVIRONMENT=production`):
- `DATABASE_URL` must include `sslmode=require` — startup fails otherwise
- HTTPS is expected at the reverse proxy layer (OVLT terminates plain HTTP internally)

Security headers set on **every response**:

| Header | Value |
|--------|-------|
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` |
| `Content-Security-Policy` | `default-src 'self'` |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |

## Rate limiting

- Per-IP fixed-window rate limiting applied to public auth endpoints (`/auth/*`)
- **PostgreSQL-backed** — counters live in `rate_limit_buckets`; safe across multiple replicas sharing the same database instance
- Limit: **20 requests per 60-second window** per IP address
- Single atomic `INSERT ... ON CONFLICT DO UPDATE RETURNING count` — no race conditions, no double-counting across replicas
- Limits apply **before** tenant resolution — an attacker cannot use tenant validity as an oracle to enumerate tenants without burning rate limit budget
- Expired buckets are purged by the background cleanup task every 6 hours

## Admin API

- All admin endpoints require `X-OVLT-Admin-Key` header
- If `OVLT_ADMIN_KEY` is not configured, admin endpoints return `404` — not `401` — to prevent endpoint enumeration
- The admin key never appears in JWT claims, audit logs, or API responses

## Encryption at rest

AES-256-GCM double-envelope encryption for all sensitive fields (TOTP secrets, token seeds). Each tenant has a unique data key derived from env vars that are never stored in the database. See [Architecture](/docs/architecture) for the full key hierarchy.

Decrypted tenant data keys are cached in memory for up to **5 minutes** (`TENANT_KEY_TTL`) to avoid re-decrypting on every request. Cached values are wrapped in `Zeroizing<String>` (via the `zeroize` crate) so the key bytes are overwritten with zeros when the cache entry expires or is evicted — they do not linger in heap memory.

## Multi-tenant isolation

PostgreSQL Row-Level Security enforces tenant boundaries at the DB layer. A query executing in the wrong tenant context returns zero rows — not a `403`. Application-level bugs cannot leak cross-tenant data because the database enforces it independently.

The `TenantDb` Axum extractor (`src/extractors.rs`) adds a second layer of enforcement at compile time. User-facing handlers that declare `TenantDb` in their signature are guaranteed to receive a `DatabaseTransaction` with `SET LOCAL ROLE ovlt_rls` and `app.tenant_id` already set — it is structurally impossible to skip this step and still reach the handler body.

## CORS

- Wildcard `*` is allowed only in development
- Starting with `ENVIRONMENT=production` and `CORS_ALLOWED_ORIGINS=*` causes an immediate startup failure
- Set `CORS_ALLOWED_ORIGINS` to an explicit comma-separated origin list

## Supply chain

- SBOM generated on every `main` push (Syft, SPDX format) — attached to GitHub Releases
- Container image scanned for CVEs (Grype) on every `main` push — critical CVEs fail the build
- SARIF results uploaded to the GitHub Security tab
- Runtime image is `gcr.io/distroless/cc-debian12:nonroot` — no shell, no package manager, no setuid binaries; only the binary + required shared libs are present

## Key rotation

OVLT supports zero-downtime key rotation via grace-period env vars. Set the old secret as the `_PREVIOUS` variant before restarting, then remove it once all in-flight tokens have expired.

### HS256 access tokens

| Env var | Role |
|---------|------|
| `JWT_SECRET` | Active signing key — all new tokens use this |
| `JWT_SECRET_PREVIOUS` | Optional; accepted during validation if current key fails |

Rotation procedure:
1. Set `JWT_SECRET_PREVIOUS=<old value>`, update `JWT_SECRET=<new value>`, restart.
2. Wait for the maximum access token TTL (default 15 min) to elapse.
3. Remove `JWT_SECRET_PREVIOUS` and restart.

### RS256 id_tokens (OIDC)

| Env var | Role |
|---------|------|
| `RSA_PRIVATE_KEY` | Active signing key — new id_tokens use this |
| `RSA_PRIVATE_KEY_PREVIOUS` | Optional; both public keys appear in `/.well-known/jwks.json` |

Rotation procedure:
1. Generate a new RSA-2048 keypair, base64-encode the PKCS8 PEM.
2. Set `RSA_PRIVATE_KEY_PREVIOUS=<old value>`, update `RSA_PRIVATE_KEY=<new value>`, restart.
3. OIDC clients will see both keys in JWKS and validate tokens by `kid` — no client-side change needed.
4. After old id_tokens expire (same TTL as access tokens), remove `RSA_PRIVATE_KEY_PREVIOUS` and restart.

## MFA backup codes

When a user enables TOTP, they can generate a set of **10 single-use recovery codes** via `POST /auth/mfa/backup-codes` (requires a valid TOTP code to confirm identity before issuing codes).

- Each code is 8 alphanumeric characters (`XXXX-XXXX` format, 40 bits of entropy)
- Stored as SHA-256 hashes — plaintext is never persisted
- Accepted at `POST /auth/mfa/challenge` via the `backup_code` field instead of `code`
- Each code is permanently invalidated after a single use (`used_at` is set)
- Generating a new set invalidates all previous codes atomically
- Disabling TOTP (user or admin) purges all backup codes for that user

## Audit log

All mutating operations are recorded in `audit_log` (per-tenant, RLS-isolated). Read-only requests are not logged. Each entry includes:

| Field | Description |
|-------|-------------|
| `action` | Dot-notation event name (e.g. `login.success`, `user.created`) |
| `user_id` | Actor UUID — the JWT `sub` of the caller, or `null` for static admin-key requests |
| `ip` | Client IP for auth events (encoded in `metadata`) |
| `metadata` | JSON object with event-specific context (target IDs, names, etc.) |
| `created_at` | UTC timestamp |

Events logged (mutations only):

| Category | Events |
|----------|--------|
| Auth | `login.success`, `login.failed.wrong_password`, `login.failed.unknown_email`, `login.locked`, `login.webauthn.success`, `user.logout`, `user.registered`, `user.password.reset` |
| MFA | `mfa.enabled`, `mfa.disabled`, `mfa.backup_codes.generated`, `mfa.admin.disabled` |
| Admin — tenants | `tenant.created` |
| Admin — clients | `client.created`, `client.updated`, `client.deactivated` |
| Admin — users | `user.created`, `user.updated`, `user.deactivated` |
| Admin — roles | `role.created`, `role.updated`, `role.deleted`, `user.role.assigned`, `user.role.revoked`, `client.role.assigned`, `client.role.revoked` |
| Admin — permissions | `permission.created`, `permission.updated`, `permission.deleted`, `role.permission.assigned`, `role.permission.revoked` |
| Admin — sessions | `session.deleted` |
| Admin — SMTP | `smtp.updated` |
| Admin — IdP | `idp.created`, `idp.updated`, `idp.deleted` |
| Admin — WebAuthn | `passkey.deleted` |

**Actor attribution** — admin events record the JWT `sub` of the Bearer token if present. Requests authenticated only via `X-OVLT-Admin-Key` record `user_id = null`.

**Export** — `GET /audit-log?limit=N` (max 10,000) returns the most recent N entries ordered by time descending. In the TUI, press `x` in the Audit Log tab to export up to 10,000 entries as a CSV file written to `~/ovlt-audit-<tenant-id>-<unix-ts>.csv`.

## Threat model

| Threat | Mitigation |
|--------|-----------|
| Brute-force passwords | Argon2id + per-tenant account lockout |
| Password reuse | History check against last N Argon2id hashes (configurable per tenant) |
| Token replay | JTI blocklist, short access token expiry |
| Stolen refresh token | Rotation on use + revocation endpoint |
| Lost authenticator app | MFA backup codes (single-use, hashed at rest) |
| Cross-tenant data access | PostgreSQL RLS at DB layer |
| Plaintext secrets at rest | AES-256-GCM double-envelope via hefesto |
| Lost encryption keys | Auto-generated + printed on first run; must be saved |
| Admin API enumeration | Key-gated; returns `404` if unconfigured |
| Clickjacking | `X-Frame-Options: DENY` |
| Supply chain attack | SBOM + Grype scan on every release; distroless runtime minimises installed attack surface |

## Production hardening checklist

<Check>`OVLT_ADMIN_KEY` set to a strong random value (32+ chars)</Check>
<Check>`JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY` saved and pinned in env</Check>
<Check>`RSA_PRIVATE_KEY` set (prevents silent key rotation on restart)</Check>
<Check>`ENVIRONMENT=production`</Check>
<Check>`DATABASE_URL` includes `sslmode=require`</Check>
<Check>`CORS_ALLOWED_ORIGINS` set explicitly (no wildcard)</Check>
<Check>`OVLT_ISSUER` set to your HTTPS public URL</Check>
<Check>TLS termination at reverse proxy (nginx, Caddy, Traefik, etc.)</Check>
<Check>Container runs as non-root (distroless image enforces UID 65532; no shell or package manager in runtime image)</Check>
<Check>PostgreSQL access restricted to the `ovlt_rls` role only</Check>
