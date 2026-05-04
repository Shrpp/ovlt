---
title: API Reference
description: All HTTP endpoints exposed by ovlt-core.
---

**Base URL:** `http://localhost:3000` (or your `OVLT_ISSUER`)

### Authentication headers

| Header | Required on |
|--------|-------------|
| `X-OVLT-Admin-Key: <OVLT_ADMIN_KEY>` | All `/admin/*` and management endpoints |
| `X-Tenant-Slug: <slug>` | All tenant-scoped endpoints |
| `Authorization: Bearer <access_token>` | Protected user endpoints |

---

## Health

```http
GET /health
```

```json
{ "status": "ok", "version": "0.1.0" }
```

---

## OIDC Discovery

```http
GET /.well-known/openid-configuration
GET /.well-known/jwks.json
```

Standard OIDC discovery document and JWK Set for verifying RS256-signed tokens.

---

## Auth

All endpoints below require `X-Tenant-Slug`.

### Register

```http
POST /auth/register
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "Secret1234!"
}
```

### Login

```http
POST /auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "Secret1234!"
}
```

```json
{
  "access_token": "eyJ...",
  "refresh_token": "...",
  "token_type": "Bearer",
  "expires_in": 900
}
```

<Note>
  When MFA is enabled on the account, login returns `{"mfa_required": true}` instead. Complete the flow via `POST /auth/mfa/challenge`.
</Note>

### Refresh token

```http
POST /auth/refresh
Content-Type: application/json

{ "refresh_token": "..." }
```

### Logout

```http
POST /auth/logout
Authorization: Bearer <token>
```

Invalidates the current session and revokes associated tokens.

### Revoke

```http
POST /auth/revoke
Authorization: Bearer <token>
Content-Type: application/json

{ "token": "<refresh_token>" }
```

### Forgot password

```http
POST /auth/forgot-password
Content-Type: application/json

{ "email": "user@example.com" }
```

Always returns `200` — no enumeration of registered emails.

### Reset password

```http
POST /auth/reset-password
Content-Type: application/json

{
  "token": "<reset_token>",
  "password": "NewSecret1234!"
}
```

### Verify email

```http
POST /auth/verify-otp
Content-Type: application/json

{ "code": "123456" }
```

### Social login

```http
GET /auth/google
GET /auth/github
```

Redirects to the provider. Callback is handled at `GET /auth/callback/{provider}`.

---

## MFA

All require `X-Tenant-Slug`. Setup and disable require `Authorization: Bearer`.

### Start MFA setup

```http
POST /auth/mfa/setup
Authorization: Bearer <token>
```

Returns `{ "secret": "...", "qr_code": "data:image/png;base64,..." }`.

### Confirm MFA setup

```http
POST /auth/mfa/confirm
Authorization: Bearer <token>
Content-Type: application/json

{ "totp_code": "123456" }
```

### MFA login challenge

```http
POST /auth/mfa/challenge
Content-Type: application/json

{
  "email": "user@example.com",
  "totp_code": "123456"
}
```

### Disable MFA (self-service)

```http
POST /auth/mfa/disable
Authorization: Bearer <token>
Content-Type: application/json

{ "totp_code": "123456" }
```

---

## OIDC / OAuth 2.0

### Authorization endpoint

```http
GET /oauth/authorize
  ?response_type=code
  &client_id=<client_id>
  &redirect_uri=<uri>
  &scope=openid email profile
  &state=<random>
  &code_challenge=<S256_hash>
  &code_challenge_method=S256
```

PKCE is required for public clients.

### Token endpoint

```http
POST /oauth/token
Content-Type: application/x-www-form-urlencoded
```

<Tabs>
  <Tab title="Authorization Code + PKCE">
    ```
    grant_type=authorization_code
    &code=<code>
    &redirect_uri=<uri>
    &client_id=<id>
    &code_verifier=<verifier>
    ```
  </Tab>
  <Tab title="Client Credentials (M2M)">
    ```
    grant_type=client_credentials
    &client_id=<id>
    &client_secret=<secret>
    &scope=<optional>
    ```
  </Tab>
  <Tab title="Refresh Token">
    ```
    grant_type=refresh_token
    &refresh_token=<token>
    &client_id=<id>
    ```
  </Tab>
</Tabs>

### Introspect

```http
POST /oauth/introspect
Content-Type: application/x-www-form-urlencoded

token=<access_token>
```

### Revoke (RFC 7009)

```http
POST /oauth/revoke
Content-Type: application/x-www-form-urlencoded

token=<refresh_token>
```

---

## Current user

```http
GET  /users/me
Authorization: Bearer <token>

PUT  /users/me/password
Authorization: Bearer <token>
Content-Type: application/json

{ "current_password": "...", "new_password": "..." }
```

---

## Admin — Tenants

Require `X-OVLT-Admin-Key`.

```http
POST  /tenants          # create tenant
GET   /tenants          # list tenants
GET   /tenants/slugs    # list slugs only
GET   /tenants/:id      # get tenant
PATCH /tenants/:id      # update tenant
DELETE /tenants/:id     # delete tenant
```

---

## Admin — Clients

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
POST   /clients          # create client
GET    /clients          # list clients
PUT    /clients/:id      # update client
DELETE /clients/:id      # deactivate client
```

---

## Admin — Users

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET    /users                           # list users
POST   /users                           # create user
GET    /users/:id                       # get user
PUT    /users/:id                       # update user
DELETE /users/:id                       # delete user
GET    /users/:id/verification-code     # get email verification code
GET    /users/:id/password-reset-token  # get password reset token
DELETE /users/:id/mfa                   # admin disable MFA
```

---

## Admin — Roles

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET    /roles                                # list roles
POST   /roles                                # create role
PUT    /roles/:id                            # update role
DELETE /roles/:id                            # delete role

GET    /users/:id/roles                      # list user roles
POST   /users/:id/roles                      # assign role to user
DELETE /users/:user_id/roles/:role_id        # revoke user role

GET    /clients/:id/roles                    # list client roles (M2M)
POST   /clients/:id/roles                    # assign role to client
DELETE /clients/:client_id/roles/:role_id    # revoke client role
```

---

## Admin — Permissions

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET    /permissions                              # list permissions
POST   /permissions                              # create permission
PUT    /permissions/:id                          # update permission
DELETE /permissions/:id                          # delete permission

GET    /roles/:id/permissions                    # list role permissions
POST   /roles/:id/permissions                    # assign permission to role
DELETE /roles/:role_id/permissions/:perm_id      # revoke permission from role
```

---

## Admin — Sessions

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET    /sessions       # list active sessions
DELETE /sessions/:id   # revoke session
```

---

## Admin — Identity Providers

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET    /identity-providers       # list providers
POST   /identity-providers       # create provider
PUT    /identity-providers/:id   # update provider
DELETE /identity-providers/:id   # delete provider
```

---

## Admin — Audit Log

Require `X-OVLT-Admin-Key` + `X-Tenant-Slug`.

```http
GET /audit-log?page=1&per_page=50
```

---

## Admin — Settings

Require `Authorization: Bearer` (admin user) + `X-Tenant-Slug`.

```http
GET /settings
PUT /settings
```
