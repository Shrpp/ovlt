---
title: Configuration
description: All environment variables for ovlt-core.
---

All configuration is via environment variables. None require a restart except secrets — those are read once at startup.

## Required

<ParamField path="DATABASE_URL" type="string" required>
  PostgreSQL connection string. Must include `sslmode=require` in production.

  ```
  postgresql://user:pass@host:5432/ovlt
  postgresql://user:pass@host:5432/ovlt?sslmode=require
  ```
</ParamField>

## Auto-generated secrets

These are generated on first run if not set. OVLT prints them to stderr. **Pin them before the second run** — losing them makes all encrypted data permanently unrecoverable.

<ParamField path="JWT_SECRET" type="string">
  HS256 signing key for access tokens. Min 32 chars.
</ParamField>

<ParamField path="MASTER_ENCRYPTION_KEY" type="string">
  AES-256-GCM master key for double-envelope encryption. Min 32 chars.
</ParamField>

<ParamField path="TENANT_WRAP_KEY" type="string">
  Wraps per-tenant data keys. Min 32 chars. **Must differ from `MASTER_ENCRYPTION_KEY`.**
</ParamField>

## Bootstrap (first-run only)

These are consumed once during `--migrate` / first startup to seed the master tenant and admin user.

<ParamField path="OVLT_ADMIN_KEY" type="string">
  Static key required in the `X-OVLT-Admin-Key` header on all admin endpoints. If unset, admin endpoints return `404` (not `401`) to prevent enumeration.
</ParamField>

<ParamField path="OVLT_BOOTSTRAP_ADMIN_EMAIL" type="string">
  Email for the first admin user in the master tenant.
</ParamField>

<ParamField path="OVLT_BOOTSTRAP_ADMIN_PASSWORD" type="string">
  Password for the bootstrap admin. Required when email is set.
</ParamField>

<ParamField path="OVLT_BOOTSTRAP_TENANT_SLUG" type="string" default="master">
  Slug for the first tenant created on startup.
</ParamField>

## Server

<ParamField path="SERVER_HOST" type="string" default="0.0.0.0">
  Bind address.
</ParamField>

<ParamField path="SERVER_PORT" type="number" default="3000">
  Port.
</ParamField>

<ParamField path="ENVIRONMENT" type="string" default="development">
  Set to `production` to enable JSON logs, strict CORS enforcement, and `sslmode` requirement on `DATABASE_URL`.
</ParamField>

<ParamField path="OVLT_ISSUER" type="string" default="http://localhost:3000">
  Issuer URL used in OIDC discovery and the `iss` claim of id\_tokens. **Set to your public HTTPS URL in production.**
</ParamField>

## Tokens

<ParamField path="JWT_EXPIRATION_MINUTES" type="number" default="15">
  Access token lifetime in minutes.
</ParamField>

<ParamField path="REFRESH_TOKEN_EXPIRATION_DAYS" type="number" default="30">
  Refresh token lifetime in days.
</ParamField>

## CORS

<ParamField path="CORS_ALLOWED_ORIGINS" type="string" default="*">
  Comma-separated list of allowed origins. Wildcards are forbidden in production — startup fails if `ENVIRONMENT=production` and this is `*`.

  ```
  CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com
  ```
</ParamField>

## RSA key (id\_tokens)

<ParamField path="RSA_PRIVATE_KEY" type="string">
  Base64-encoded PKCS8 PEM for RS256 id\_token signing. If unset, an ephemeral keypair is generated at startup — it's lost on restart and JWKS consumers will see a new key.

  Generate a persistent key:

  ```bash
  openssl genrsa 2048 | openssl pkcs8 -topk8 -nocrypt -out key.pem
  base64 -i key.pem | tr -d '\n'
  # paste output as RSA_PRIVATE_KEY
  ```
</ParamField>

## Social login

All three vars must be set for a provider to be enabled.

<ParamField path="GOOGLE_CLIENT_ID" type="string">
  Google OAuth2 client ID.
</ParamField>
<ParamField path="GOOGLE_CLIENT_SECRET" type="string">
  Google OAuth2 client secret.
</ParamField>
<ParamField path="GOOGLE_REDIRECT_URL" type="string">
  Must match the registered redirect URI in Google Cloud Console. Example: `https://your-domain.com/auth/callback/google`.
</ParamField>

<ParamField path="GITHUB_CLIENT_ID" type="string">
  GitHub OAuth2 client ID.
</ParamField>
<ParamField path="GITHUB_CLIENT_SECRET" type="string">
  GitHub OAuth2 client secret.
</ParamField>
<ParamField path="GITHUB_REDIRECT_URL" type="string">
  Must match the registered redirect URI in GitHub OAuth App settings.
</ParamField>

## Production checklist

Before going live, verify every item:

<Check>
  `DATABASE_URL` includes `sslmode=require`
</Check>
<Check>
  `JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY` saved and pinned in env
</Check>
<Check>
  `OVLT_ISSUER` set to your public HTTPS URL
</Check>
<Check>
  `ENVIRONMENT=production`
</Check>
<Check>
  `CORS_ALLOWED_ORIGINS` set to an explicit list (no wildcard)
</Check>
<Check>
  `RSA_PRIVATE_KEY` set (prevents key rotation on restart)
</Check>
<Check>
  `OVLT_ADMIN_KEY` set to a strong random value
</Check>
<Check>
  TLS termination configured at reverse proxy (nginx, Caddy, etc.)
</Check>
