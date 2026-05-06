---
title: Database Access
description: Connect to the OVLT PostgreSQL database from DataGrip, DBeaver, or any SQL client.
---

OVLT stores all data in PostgreSQL. You can connect directly with any SQL client for inspection, debugging, or manual queries.

<Warning>
  Direct database access bypasses Row-Level Security (RLS) when connecting as the `ovlt` superuser. Only use this for administration and debugging — never in application code.
</Warning>

## Connection details

| Field | Value |
|-------|-------|
| Driver | PostgreSQL |
| Host | See below |
| Port | `5432` |
| Database | `ovlt` |
| Username | `ovlt` |
| Password | `ovlt` (default in docker-compose) |

---

## Local setup

If you're running `docker compose up` on your own machine, PostgreSQL is exposed on `localhost:5432`.

<Steps>
  <Step title="Open your SQL client">
    In **DataGrip**: `New → Data Source → PostgreSQL`

    In **DBeaver**: `Database → New Database Connection → PostgreSQL`
  </Step>
  <Step title="Fill in the connection details">
    ```
    Host:     localhost
    Port:     5432
    Database: ovlt
    User:     ovlt
    Password: ovlt
    ```
  </Step>
  <Step title="Test the connection">
    Click **Test Connection**. If it fails, verify the container is running:

    ```bash
    docker ps | grep ovlt-postgres
    ```
  </Step>
</Steps>

---

## VPS / Remote server

PostgreSQL is not exposed to the internet by default — it only listens on the container's internal network. To connect remotely, use an **SSH tunnel**.

<Steps>
  <Step title="Open the SSH tunnel">
    Run this on your local machine. Replace `user` and `your-server-ip` with your SSH credentials:

    ```bash
    ssh -N -L 5432:localhost:5432 user@your-server-ip
    ```

    Keep this terminal open — the tunnel stays active while the command runs.

    <Tip>
      Add `-f` to run the tunnel in the background:
      ```bash
      ssh -f -N -L 5432:localhost:5432 user@your-server-ip
      ```
      To close it later: `pkill -f "ssh -f -N -L 5432"`
    </Tip>
  </Step>
  <Step title="Connect your SQL client to localhost">
    With the tunnel open, your SQL client connects to `localhost:5432` as if PostgreSQL were running locally:

    ```
    Host:     localhost
    Port:     5432
    Database: ovlt
    User:     ovlt
    Password: ovlt
    ```

    The tunnel forwards all traffic securely through SSH to the remote server.
  </Step>
</Steps>

<Note>
  If your server uses a non-standard SSH port, specify it with `-p`:
  ```bash
  ssh -N -L 5432:localhost:5432 -p 2222 user@your-server-ip
  ```
</Note>

---

## Key tables

| Table | Description |
|-------|-------------|
| `tenants` | Tenant registry — one row per realm |
| `users` | Users per tenant. `email_enc` is AES-256-GCM encrypted; use `email_lookup` (HMAC hash) for querying |
| `oauth_clients` | OAuth 2.0 clients per tenant |
| `sessions` | Active user sessions |
| `refresh_tokens` | Hashed refresh tokens (plaintext never stored) |
| `roles` / `permissions` | RBAC definitions |
| `user_roles` | Role assignments per user |
| `audit_log` | Auth event log (login, failures, registration) |
| `webauthn_credentials` | Registered passkeys per user |
| `tenant_smtp_config` | Per-tenant SMTP — `password_enc` is encrypted |
| `one_time_tokens` | Email verification and password-reset tokens (hashed) |

---

## Useful queries

```sql
-- List all tenants
SELECT id, slug, name, plan, is_active, created_at FROM tenants;

-- Count users per tenant
SELECT t.slug, COUNT(u.id) AS users
FROM tenants t
LEFT JOIN users u ON u.tenant_id = t.id
GROUP BY t.slug;

-- Recent audit events
SELECT a.action, a.ip, a.created_at, t.slug AS tenant
FROM audit_log a
JOIN tenants t ON t.id = a.tenant_id
ORDER BY a.created_at DESC
LIMIT 50;

-- Active sessions
SELECT s.id, t.slug, s.created_at, s.last_seen_at, s.expires_at
FROM sessions s
JOIN tenants t ON t.id = s.tenant_id
WHERE s.expires_at > NOW()
ORDER BY s.last_seen_at DESC;

-- List passkeys
SELECT wc.name, wc.created_at, wc.last_used_at, t.slug
FROM webauthn_credentials wc
JOIN tenants t ON t.id = wc.tenant_id;
```

<Warning>
  Email addresses are stored encrypted (`email_enc`). You cannot read them directly from the database without the `MASTER_ENCRYPTION_KEY` and `TENANT_WRAP_KEY`. The `email_lookup` column is an HMAC hash used for lookups — it is not the email address.
</Warning>
