---
title: M2M / Client Credentials
description: Authenticate services and scripts against OVLT using the OAuth 2.0 client_credentials grant.
---

Machine-to-machine (M2M) auth uses the OAuth 2.0 `client_credentials` grant. No user is involved — a service authenticates with its client ID and secret, and receives a signed JWT with embedded roles.

<Steps>
  <Step title="Create an M2M client">
    <Tabs>
      <Tab title="Via TUI">
        Open the **Clients** tab → press `n` → set **Grant Types** to `client_credentials` only (do not include `authorization_code`).
      </Tab>
      <Tab title="Via API">
        ```bash
        curl -X POST http://localhost:3000/clients \
          -H "X-OVLT-Admin-Key: your-admin-key" \
          -H "X-Tenant-Slug: master" \
          -H "Content-Type: application/json" \
          -d '{
            "name": "my-service",
            "grant_types": ["client_credentials"]
          }'
        ```
      </Tab>
    </Tabs>

    The response includes `client_id` and `client_secret`.

    <Warning>
      Save the `client_secret` immediately — it is shown once and cannot be retrieved again.
    </Warning>
  </Step>

  <Step title="Assign roles (optional)">
    Roles are embedded in the token and let downstream services perform RBAC checks without a DB lookup.

    <Tabs>
      <Tab title="Via TUI">
        Select the client → press `r` → assign roles from the list.
      </Tab>
      <Tab title="Via API">
        ```bash
        # List available roles
        curl http://localhost:3000/roles \
          -H "X-OVLT-Admin-Key: your-admin-key" \
          -H "X-Tenant-Slug: master"

        # Assign a role
        curl -X POST http://localhost:3000/clients/<client_id>/roles \
          -H "X-OVLT-Admin-Key: your-admin-key" \
          -H "X-Tenant-Slug: master" \
          -H "Content-Type: application/json" \
          -d '{"role_id": "<role_uuid>"}'
        ```
      </Tab>
    </Tabs>
  </Step>

  <Step title="Request a token">
    ```bash
    curl -X POST http://localhost:3000/oauth/token \
      -H "X-Tenant-Slug: master" \
      -d "grant_type=client_credentials" \
      -d "client_id=<client_id>" \
      -d "client_secret=<client_secret>"
    ```

    Response:

    ```json
    {
      "access_token": "eyJ...",
      "token_type": "Bearer",
      "expires_in": 900
    }
    ```
  </Step>

  <Step title="Verify the token in your service">
    Use the JWKS endpoint to verify the RS256 signature. Most JWT libraries support auto-discovery via the OpenID configuration endpoint.

    ```
    GET http://localhost:3000/.well-known/openid-configuration
    GET http://localhost:3000/.well-known/jwks.json
    ```

    <Tabs>
      <Tab title="Node.js (jose)">
        ```js
        import { createRemoteJWKSet, jwtVerify } from 'jose';

        const JWKS = createRemoteJWKSet(
          new URL('http://localhost:3000/.well-known/jwks.json')
        );

        const { payload } = await jwtVerify(token, JWKS, {
          issuer: 'http://localhost:3000',
          audience: 'ovlt',
        });

        console.log(payload.roles); // ["admin", "data-reader"]
        ```
      </Tab>
      <Tab title="Python (python-jose)">
        ```python
        from jose import jwt
        import httpx

        jwks = httpx.get('http://localhost:3000/.well-known/jwks.json').json()
        payload = jwt.decode(token, jwks, algorithms=['RS256'],
                             audience='ovlt', issuer='http://localhost:3000')
        print(payload['roles'])
        ```
      </Tab>
    </Tabs>
  </Step>
</Steps>

## Token payload

```json
{
  "sub": "<client_id>",
  "iss": "http://localhost:3000",
  "aud": "ovlt",
  "exp": 1714300000,
  "iat": 1714299100,
  "jti": "<uuid>",
  "client_id": "<client_id>",
  "tenant_id": "<uuid>",
  "roles": ["admin", "data-reader"]
}
```

`roles` is omitted when no roles are assigned to the client.

Decode locally for debugging (no signature verification):

```bash
echo "eyJ..." | cut -d. -f2 | base64 -d 2>/dev/null | jq
```

## Flow diagram

```
Service                         OVLT
  │                               │
  │  POST /oauth/token             │
  │  grant_type=client_credentials│
  │ ─────────────────────────────>│
  │                               │ verify secret, load roles
  │       access_token (JWT)      │
  │ <─────────────────────────────│
  │                               │
  │  downstream API call          │
  │  Authorization: Bearer <jwt>  │
  │ ─────────────────────────────>│ verify RS256 via JWKS
```
