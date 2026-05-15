---
title: Security
description: Security model, threat model, cryptographic primitives, and hardening guide for OVLT.
---

# Security

This document describes OVLT's security model, the cryptographic primitives it uses, what is and isn't protected, and how to deploy it safely. It is the source of truth — if marketing copy on the website ever conflicts with this page, this page wins.

> **Alpha disclaimer.** OVLT is in alpha and has not been externally audited. Use it for evaluation, internal tools, and side projects. Do not use it in production until the stable beta is released (target Q3 2026) and an audit has been published.

---

## What OVLT protects against

| Threat | Protection |
| --- | --- |
| Password brute-force | Argon2id hashing (19 MB memory, 2 iterations) + per-tenant account lockout (5 attempts / 15 min) |
| Database backup theft | AES-256-GCM envelope encryption for sensitive fields (TOTP secrets, SMTP creds, IdP secrets) |
| Cross-tenant data access via app bug | PostgreSQL Row-Level Security at the database layer — independent of application code |
| Stolen access token | Short TTL (15 min default) + JTI blocklist for early revocation |
| Stolen refresh token | Rotation on every use + detect-and-revoke on reuse |
| Token replay | JTI tracked in DB, rejected at introspection |
| Clickjacking | `X-Frame-Options: DENY` on every response |
| MIME confusion | `X-Content-Type-Options: nosniff` |
| Cookie theft via HTTP | `Secure` flag enforced in production |
| Tenant enumeration | Rate limit before tenant resolution; auth endpoints respond identically for unknown vs known users (timing-safe) |
| Admin endpoint enumeration | Returns `404` (not `401`) when `OVLT_ADMIN_KEY` is not configured |
| Supply chain attack | SBOM (Syft, SPDX) + container scan (Grype) on every release; SARIF uploaded to GitHub Security |

## What OVLT does NOT protect against

Honesty matters more than marketing. These are explicitly out of scope or not yet implemented:

- **Server compromise with read access to env vars.** `JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, and `TENANT_WRAP_KEY` live in environment variables. An attacker who reads the process environment can decrypt all data at rest. Mitigation: run OVLT in a hardened container with restricted access. KMS-backed key management is on the roadmap (Stage 8+).
- **Plaintext credentials in server memory during auth.** The current implementation processes user passwords in plaintext during the auth flow. This will change with OPAQUE (Stage 7) — see [Roadmap](#roadmap-and-current-state) below.
- **DoS via massive traffic.** OVLT has per-IP rate limiting but is not designed to absorb a sustained DDoS. Use a reverse proxy or CDN with DDoS protection.
- **Cryptographic attacks on AES-256-GCM.** OVLT inherits the security properties of the underlying primitive. If AES-256-GCM is broken, OVLT is broken.
- **Compromised TLS termination.** OVLT terminates HTTP internally and expects HTTPS at the reverse proxy. If TLS is misconfigured upstream, traffic is exposed.
- **Insider threat from the deploying operator.** A malicious operator with shell access and env var visibility can decrypt all stored data. This is a property of the deployment, not a flaw — but worth stating.
- **Social engineering, phishing, password reuse by users.** Out of scope for the server; mitigated by good user education and MFA enforcement.

---

## Cryptographic primitives

| Use | Primitive | Notes |
| --- | --- | --- |
| Password hashing | Argon2id | 19 MB memory, 2 iterations, 1 thread (OWASP minimum). Planned replacement: OPAQUE in v0.5 |
| `id_token` signing | RS256 (RSA-PSS-2048) | Published via JWKS endpoint at `/.well-known/jwks.json` |
| Access token signing | HS256 | Symmetric, server-internal only; not intended to be verified by third parties |
| Encryption at rest | AES-256-GCM | Double-envelope via hefesto; per-tenant data keys derived from master + wrap keys |
| TLS | Not implemented by OVLT | Expected at reverse proxy (nginx, Caddy, Traefik) |
| MFA codes | HMAC-SHA1 (RFC 6238) | TOTP standard; 30-second window, 6 digits |
| Passkey assertion | WebAuthn / FIDO2 Level 2 | Browser-driven; ES256 and EdDSA verifier supported |

### About the HS256 access token choice

OVLT signs `id_tokens` with RS256 (asymmetric) so third parties — frontend clients, API gateways, downstream services — can verify token authenticity via the public JWKS endpoint without contacting OVLT.

Access tokens are signed with HS256 (symmetric) because they are intended to be opaque to third parties and verified only by OVLT itself via the `/oauth/introspect` endpoint. Using HS256 here is a deliberate choice for short-lived tokens that should not be self-validated.

If you need access tokens that can be verified offline by other services, use the `id_token` (RS256) for that purpose, or configure your resource server to call `/oauth/introspect`.

---

## Passwords

All passwords are hashed with **Argon2id** before storage:

- Parameters: 19 MB memory, 2 iterations, 1 thread (OWASP recommended minimum)
- Plaintext never stored, logged, or returned in any response
- Resistant to GPU and ASIC brute-force attacks within practical compute budgets

The current implementation does see passwords in plaintext in server memory during the auth request lifecycle. This is the same operational property as Keycloak, Authentik, and most OIDC servers — but it is not zero-knowledge. We do not claim it is.

**OPAQUE migration (planned for v0.5):** OVLT will integrate OPAQUE (an aPAKE protocol via the `opaque-ke` crate) so the server never sees the password, even during login. The server will hold only a one-way verifier that cannot be used to recover or guess the password, even given full database access. Existing Argon2id users will be migrated on next successful login. See [Roadmap and current state](#roadmap-and-current-state) for the schedule.

---

**Password history** — when `history_size > 0` in a tenant's password policy, the last N hashes are checked on every password change (reset flow and admin-forced change). Reusing a recent password returns a 400 error. Each accepted hash is recorded in `password_history` (RLS-isolated per tenant). Setting `history_size = 0` disables the check.

## Account lockout

After **5 consecutive failed login attempts**, the account is locked for **15 minutes**.

- Lockout is per-user, per-tenant — one tenant's lockouts do not affect another
- Both thresholds are configurable per-tenant via Settings
- Stale attempt records purged every 6 hours by the background cleanup task
- Lockout does not reveal account existence to unauthenticated requesters (responds identically to "no such user")

---

## Token security

| Token | Algorithm | TTL | Storage | Notes |
| --- | --- | --- | --- | --- |
| Access token | HS256 JWT | 15 min default | DB (JTI) | JTI blocklist enforced at introspection |
| Refresh token | Opaque random | Configurable | DB (hashed) | Rotated on every use; reuse triggers session invalidation |
| `id_token` | RS256 JWT | Same as access | Not stored | Issued only on `authorization_code` flow |

- **JTI blocklist** — replayed access tokens are rejected at introspection even before expiry.
- **Refresh token rotation** — a stolen refresh token used by an attacker invalidates the legitimate user's session immediately (detect-and-revoke).
- **Revocation** — `/auth/revoke` and `/oauth/revoke` propagate immediately; no cache delay.

---

## Transport security

In production (`ENVIRONMENT=production`):

- `DATABASE_URL` must include `sslmode=require` — startup fails otherwise.
- HTTPS expected at the reverse proxy layer. OVLT terminates plain HTTP internally.
- Cookies set with `Secure`, `HttpOnly`, and `SameSite=Lax`. Startup fails if `ENVIRONMENT=production` and `OVLT_COOKIE_SECURE=false`.

Security headers set on **every response**:

| Header | Value |
| --- | --- |
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` |
| `Content-Security-Policy` | `default-src 'self'` |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |

---

## Rate limiting

- Per-IP sliding-window rate limiting on public auth endpoints (`/auth/*`).
- Limits apply **before** tenant resolution — tenant validity cannot be used as an enumeration oracle without burning the rate limit budget.
- Limits are currently per-process. A distributed rate limiter (PostgreSQL-backed, multi-replica safe) is in Stage 5 and will be enforced before any deployment guidance recommends multi-replica setups.

---

## Multi-tenant isolation

PostgreSQL Row-Level Security enforces tenant boundaries at the database layer. A query executing in the wrong tenant context returns zero rows — not a `403`. Application-level bugs cannot leak cross-tenant data because the database enforces the boundary independently.

A type-safe RLS extractor is on the Stage 5 roadmap. Once implemented, any database query that does not go through the tenant-scoped extractor will fail to compile, making RLS bypass a compile-time error rather than a code-review concern.

---

## Admin API

- All admin endpoints require the `X-OVLT-Admin-Key` header.
- If `OVLT_ADMIN_KEY` is not configured, admin endpoints return `404` — not `401` — to prevent endpoint enumeration via response codes.
- The admin key never appears in JWT claims, audit logs, or API responses.
- Admin actions are recorded in the audit log with `actor: admin` and a hash of the admin key (not the key itself) for correlation.

---

## Encryption at rest

OVLT uses [hefesto](https://crates.io/crates/hefesto) for AES-256-GCM envelope encryption of sensitive fields:

- TOTP secrets
- SMTP credentials (per-tenant)
- IdP client secrets (Google, GitHub OAuth client secrets per tenant)
- Webhook signing keys
- Refresh token seeds

**Key hierarchy:**

```
MASTER_ENCRYPTION_KEY   (env var, 32 bytes, never stored in DB)
        │
        ▼
TENANT_WRAP_KEY         (env var, 32 bytes, never stored in DB)
        │
        ▼ derives
Tenant data key         (per-tenant, generated on tenant creation)
        │
        ▼ encrypts
Sensitive field         (TOTP secret, SMTP password, etc.)
```

If the master or wrap key is lost, all encrypted data is permanently inaccessible. Save the auto-generated keys printed on first run.

**Planned for v0.5 (Stage 7):** client-side envelope encryption for tenant configuration secrets. The admin's browser will encrypt SMTP passwords and IdP secrets with a key derived from their password before transmission. The server will hold only ciphertext at rest and will not be able to decrypt without an active admin session.

See [Architecture](architecture.md) for the full encryption model.

---

The `TenantDb` Axum extractor (`src/extractors.rs`) adds a second layer of enforcement at compile time. User-facing handlers that declare `TenantDb` in their signature are guaranteed to receive a `DatabaseTransaction` with `SET LOCAL ROLE ovlt_rls` and `app.tenant_id` already set — it is structurally impossible to skip this step and still reach the handler body.

## CORS

- Wildcard `*` is allowed only in development.
- Starting with `ENVIRONMENT=production` and `CORS_ALLOWED_ORIGINS=*` causes an immediate startup failure.
- Set `CORS_ALLOWED_ORIGINS` to an explicit comma-separated origin list.

---

## Supply chain

- SBOM generated on every `main` push (Syft, SPDX format) — attached to GitHub Releases.
- Container image scanned for CVEs (Grype) on every `main` push — critical CVEs fail the build.
- SARIF results uploaded to the GitHub Security tab.
- `cargo audit` enforced in CI; PRs that introduce vulnerable dependencies are blocked.
- `cargo deny` enforced for license compatibility and duplicate detection.

---

## Reporting a vulnerability

**Do not open a public GitHub issue for security reports.**

Email `me@shrpp.dev` with subject `SECURITY: <short description>`.

Response commitments:

| Severity | Acknowledgment | Assessment | Fix or mitigation plan |
| --- | --- | --- | --- |
| Critical | 24 hours | 3 days | 7 days |
| High | 48 hours | 5 days | 14 days |
| Medium | 48 hours | 7 days | 30 days |
| Low | 5 days | 14 days | Next release |

After the fix ships, a public advisory is published in the [Security tab](https://github.com/Shrpp/ovlt/security) with credit to the reporter (with consent).

### Scope

**In scope:**
- All code in the `Shrpp/ovlt` repository
- The `hefesto` crate
- The default deployment configuration (`docker-compose.yml`, `Dockerfile`)

**Out of scope:**
- The ovlt.tech marketing website
- Social engineering attacks against the maintainer
- DoS via massive traffic volume (use a reverse proxy with appropriate limits)
- Vulnerabilities in third-party dependencies (please report to upstream; we will track via `cargo audit`)

---

## Disclosed vulnerabilities

| Advisory | Severity | Affected versions | Fixed in | Reporter |
| --- | --- | --- | --- | --- |
| _(none disclosed at time of writing)_ | | | | |

This table will be updated as advisories are published.

---

## Audit status

- **Application code:** Unaudited. External audit planned ahead of v1.0 stable.
- **Hefesto crate:** Unaudited. Threat model documented in the hefesto repository.
- **Dependencies:** Tracked via `cargo audit` in CI; no known unpatched vulnerabilities.

If you are interested in funding or contributing to an external audit (Trail of Bits, NCC Group, Cure53, or equivalent), reach out via the email above.

---

## Roadmap and current state

This page reflects the current state of v0.4.4-alpha. Items in progress or planned:

| Item | Stage | Status |
| --- | --- | --- |
| Cookie Secure flag in production | 5 | In progress |
| Type-safe RLS extractor | 5 | In progress |
| Distributed rate limiter (Postgres-backed) | 5 | In progress |
| MFA backup codes | 5 | In progress |
| Tenant key cache with zeroize-on-drop | 5 | In progress |
| Key rotation with grace period | 5 | In progress |
| Docker image hardening (distroless, non-root) | 5 | In progress |
| Comprehensive integration test suite | 6 | In progress |
| OPAQUE password authentication | 7 | Planned |
| Client-side encrypted tenant secrets | 7 | Planned |
| External security audit | 9+ | Planned for v1.0 |

See the [README Roadmap](../README.md#roadmap) for the full picture.

---

## Production hardening checklist

Before deploying OVLT to anything resembling production (even a small internal tool):

- [ ] `OVLT_ADMIN_KEY` set to a strong random value (32+ chars)
- [ ] `JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY` saved offline and pinned in env
- [ ] `RSA_PRIVATE_KEY` set (prevents silent key rotation on restart)
- [ ] `ENVIRONMENT=production`
- [ ] `DATABASE_URL` includes `sslmode=require`
- [ ] `OVLT_COOKIE_SECURE=true` (default in production)
- [ ] `CORS_ALLOWED_ORIGINS` set explicitly (no wildcard)
- [ ] `OVLT_ISSUER` set to your HTTPS public URL
- [ ] TLS termination at reverse proxy (nginx, Caddy, Traefik)
- [ ] Container runs as non-root (`USER 65534` in Dockerfile)
- [ ] PostgreSQL access restricted to the `ovlt_rls` role only
- [ ] Backups of the database AND the env-var key material (without these, encrypted data is unrecoverable)
- [ ] Audit log retention policy defined
- [ ] You have read the [What OVLT does NOT protect against](#what-ovlt-does-not-protect-against) section above and accept the residual risk

---

## Questions

- Security report (private): `me@shrpp.dev`
- Architecture discussion (public): [GitHub Discussions](https://github.com/shrpp/ovlt/discussions)
- General security questions: open a Discussion in the `security` category
