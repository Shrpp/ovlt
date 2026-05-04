---
title: MFA (TOTP)
description: Set up and manage TOTP-based two-factor authentication for users.
---

OVLT supports TOTP-based MFA (RFC 6238) — compatible with Google Authenticator, Authy, 1Password, and any standard TOTP app. The TOTP secret is stored encrypted at rest (AES-256-GCM double-envelope).

## User setup flow

<Steps>
  <Step title="Initiate setup">
    The user must be authenticated. Call the setup endpoint with their access token:

    ```bash
    curl -X POST http://localhost:3000/auth/mfa/setup \
      -H "Authorization: Bearer <access_token>" \
      -H "X-Tenant-Slug: your-tenant"
    ```

    Response:

    ```json
    {
      "secret": "BASE32SECRET...",
      "qr_code": "data:image/png;base64,..."
    }
    ```

    Render the `qr_code` as an `<img src="...">` in your UI, or display the `secret` for manual entry in the authenticator app.
  </Step>

  <Step title="Confirm the code">
    After the user scans the QR code and enters their first 6-digit code:

    ```bash
    curl -X POST http://localhost:3000/auth/mfa/confirm \
      -H "Authorization: Bearer <access_token>" \
      -H "X-Tenant-Slug: your-tenant" \
      -H "Content-Type: application/json" \
      -d '{"totp_code": "123456"}'
    ```

    MFA is now active on the account. The plaintext secret is discarded — it is no longer retrievable.
  </Step>

  <Step title="Login with MFA enabled">
    When MFA is active, a normal `POST /auth/login` returns a challenge instead of tokens:

    ```json
    { "mfa_required": true }
    ```

    The client must then complete the MFA challenge:

    ```bash
    curl -X POST http://localhost:3000/auth/mfa/challenge \
      -H "X-Tenant-Slug: your-tenant" \
      -H "Content-Type: application/json" \
      -d '{
        "email": "user@example.com",
        "totp_code": "123456"
      }'
    ```

    A successful challenge returns the same `access_token` / `refresh_token` pair as a normal login.
  </Step>
</Steps>

## Disable MFA

<Tabs>
  <Tab title="Self-service (user)">
    The user must provide a valid TOTP code to disable MFA:

    ```bash
    curl -X POST http://localhost:3000/auth/mfa/disable \
      -H "Authorization: Bearer <access_token>" \
      -H "X-Tenant-Slug: your-tenant" \
      -H "Content-Type: application/json" \
      -d '{"totp_code": "123456"}'
    ```
  </Tab>
  <Tab title="Admin override">
    If a user loses their authenticator device and can't log in, an admin can disable MFA for them:

    ```bash
    curl -X DELETE http://localhost:3000/users/<user_id>/mfa \
      -H "X-OVLT-Admin-Key: your-admin-key" \
      -H "X-Tenant-Slug: your-tenant"
    ```

    Via TUI: **Users** tab → select user → admin disable option.
  </Tab>
</Tabs>

## Notes

- TOTP codes are 6 digits with a 30-second window and ±1 step clock-skew tolerance
- Once MFA is confirmed, the plaintext TOTP secret is not retrievable — users must re-setup if they lose their device
- The secret is stored AES-256-GCM encrypted with the tenant's double-envelope key hierarchy
