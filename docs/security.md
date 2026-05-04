---
title: Security
description: Security model, defaults, and hardening guide for production deployments.
---

## Passwords

All passwords are hashed with **Argon2id** before storage:

- Parameters: 19 MB memory, 2 iterations, 1 thread (OWASP recommended minimum)
- Plaintext is never stored, logged, or returned in any response
- Resistant to GPU and ASIC brute-force attacks

## Account lockout

After **5 consecutive failed login attempts**, the account is locked for **15 minutes**.

- Lockout is per-user, per-tenant — one tenant's lockouts don't affect another
- Both thresholds are configurable per-tenant via Settings
- Stale attempt records are purged every 6 hours by the background cleanup task

## Token security

| Token | Details |
|-------|---------|
| Access token | RS256 JWT, 15 min default, JTI tracked in DB |
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

- Per-IP sliding-window rate limiting applied to all auth endpoints (`/auth/*`, `/oauth/*`)
- Limits apply **before** tenant resolution — blocks enumeration probes across tenants

## Admin API

- All admin endpoints require `X-OVLT-Admin-Key` header
- If `OVLT_ADMIN_KEY` is not configured, admin endpoints return `404` — not `401` — to prevent endpoint enumeration
- The admin key never appears in JWT claims, audit logs, or API responses

## Encryption at rest

AES-256-GCM double-envelope encryption for all sensitive fields (TOTP secrets, token seeds). Each tenant has a unique data key derived from env vars that are never stored in the database. See [Architecture](/docs/architecture) for the full key hierarchy.

## Multi-tenant isolation

PostgreSQL Row-Level Security enforces tenant boundaries at the DB layer. A query executing in the wrong tenant context returns zero rows — not a `403`. Application-level bugs cannot leak cross-tenant data because the database enforces it independently.

## CORS

- Wildcard `*` is allowed only in development
- Starting with `ENVIRONMENT=production` and `CORS_ALLOWED_ORIGINS=*` causes an immediate startup failure
- Set `CORS_ALLOWED_ORIGINS` to an explicit comma-separated origin list

## Supply chain

- SBOM generated on every `main` push (Syft, SPDX format) — attached to GitHub Releases
- Container image scanned for CVEs (Grype) on every `main` push — critical CVEs fail the build
- SARIF results uploaded to the GitHub Security tab

## Threat model

| Threat | Mitigation |
|--------|-----------|
| Brute-force passwords | Argon2id + per-tenant account lockout |
| Token replay | JTI blocklist, short access token expiry |
| Stolen refresh token | Rotation on use + revocation endpoint |
| Cross-tenant data access | PostgreSQL RLS at DB layer |
| Plaintext secrets at rest | AES-256-GCM double-envelope via hefesto |
| Lost encryption keys | Auto-generated + printed on first run; must be saved |
| Admin API enumeration | Key-gated; returns `404` if unconfigured |
| Clickjacking | `X-Frame-Options: DENY` |
| Supply chain attack | SBOM + Grype scan on every release |

## Production hardening checklist

<Check>`OVLT_ADMIN_KEY` set to a strong random value (32+ chars)</Check>
<Check>`JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY` saved and pinned in env</Check>
<Check>`RSA_PRIVATE_KEY` set (prevents silent key rotation on restart)</Check>
<Check>`ENVIRONMENT=production`</Check>
<Check>`DATABASE_URL` includes `sslmode=require`</Check>
<Check>`CORS_ALLOWED_ORIGINS` set explicitly (no wildcard)</Check>
<Check>`OVLT_ISSUER` set to your HTTPS public URL</Check>
<Check>TLS termination at reverse proxy (nginx, Caddy, Traefik, etc.)</Check>
<Check>Container runs as non-root (Dockerfile uses `USER 65534`)</Check>
<Check>PostgreSQL access restricted to the `ovlt_rls` role only</Check>
