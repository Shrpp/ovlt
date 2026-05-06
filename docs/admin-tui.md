---
title: Admin TUI
description: Complete reference for the ovlt terminal UI — layout, navigation, every tab, every modal.
---

The `ovlt` binary is a full-featured terminal admin interface built with Ratatui. Every action maps to an admin API call — use the [API Reference](/api-reference) for scripted automation.

## Install

<Tabs>
  <Tab title="macOS (Apple Silicon)">
    ```bash
    curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-aarch64-apple-darwin
    xattr -dr com.apple.quarantine ovlt
    chmod +x ovlt && sudo mv ovlt /usr/local/bin/
    ```
    <Note>
      `xattr -dr com.apple.quarantine` removes the Gatekeeper quarantine flag on unsigned alpha binaries.
    </Note>
  </Tab>
  <Tab title="macOS (Intel)">
    ```bash
    curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-x86_64-apple-darwin
    xattr -dr com.apple.quarantine ovlt
    chmod +x ovlt && sudo mv ovlt /usr/local/bin/
    ```
  </Tab>
  <Tab title="Linux (x86_64)">
    ```bash
    curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-x86_64-unknown-linux-gnu
    chmod +x ovlt && sudo mv ovlt /usr/local/bin/
    ```
  </Tab>
  <Tab title="Linux (ARM64)">
    ```bash
    curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-aarch64-unknown-linux-gnu
    chmod +x ovlt && sudo mv ovlt /usr/local/bin/
    ```
  </Tab>
  <Tab title="Windows">
    ```powershell
    curl -Lo ovlt.exe https://github.com/shrpp/ovlt/releases/latest/download/ovlt-x86_64-pc-windows-msvc.exe
    .\ovlt.exe --url http://localhost:3000
    ```
  </Tab>
  <Tab title="Build from source">
    ```bash
    git clone https://github.com/shrpp/ovlt
    cd ovlt
    cargo build --release -p ovlt-cli
    ./target/release/ovlt --url http://localhost:3000
    ```
  </Tab>
</Tabs>

## Launch

```bash
ovlt --url http://localhost:3000
# or via environment variable
OVLT_URL=http://localhost:3000 ovlt
```

On launch, a login screen appears centered on the terminal. Enter:
- **Email** — the bootstrap admin email (default: value of `OVLT_BOOTSTRAP_ADMIN_EMAIL`)
- **Password** — the bootstrap admin password
- **Tenant** — select from the dropdown or type a slug (default: `master`)

If MFA is enabled on the account, a TOTP challenge screen appears after login.

---

## Layout

```
┌─────────────────────────────────────────────────────────────┐
│  ovlt/                                    ● connected        │  ← header
├────────────┬────────────────────────────────────────────────┤
│            │  Clients │ Users │ Roles │ … │                 │  ← tab bar
│  Tenants   ├────────────────────────────────────────────────┤
│            │                                                 │
│  ▶ master  │              content area                       │
│    acme    │                                                 │
│    dev     │                                                 │
│            │                                                 │
├────────────┴────────────────────────────────────────────────┤
│  status bar                                                  │
└─────────────────────────────────────────────────────────────┘
```

| Region | Description |
|--------|-------------|
| **Header** | `ovlt/` brand + live health indicator (● green = connected, ● yellow = connecting, ● red = error) |
| **Sidebar** | Tenant list. Use `↑/↓` to select; all other tabs scope to the selected tenant |
| **Tab bar** | Clients · Users · Roles · Permissions · Sessions · Settings · IdP · Audit |
| **Content area** | Table or form for the active tab |
| **Status bar** | Transient feedback messages (save success, errors, etc.) |

**Focus model:** `Tab` or arrow keys move focus between the sidebar and the content area. The focused region has a **cyan border**; unfocused regions have a gray border.

---

## Global keyboard shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Move focus: sidebar ↔ content tabs |
| `←` / `→` | Switch tabs (when content is focused) |
| `↑` / `↓` or `j` / `k` | Move selection up / down in a list |
| `Enter` | Open / confirm |
| `Esc` | Close modal / cancel |
| `n` | New item |
| `d` | Delete selected |
| `e` | Edit selected |
| `r` | Manage roles (Clients tab only) |
| `?` | Quick Start wizard |
| `q` | Quit |

---

## Sidebar — Tenants

The sidebar lists all tenants. **Selecting a tenant scopes all content tabs to it.** Press `n` to create a new tenant (name + slug). Press `d` to delete (confirmation required).

---

## Content tabs

### Clients

OAuth 2.0 / OIDC clients. Each client belongs to one tenant.

| Column | Notes |
|--------|-------|
| ID | Short UUID prefix |
| Name | Display name |
| Client ID | Full `client_id` string sent in OAuth flows |
| Type | `confidential` (server-side) or `public` (SPA/mobile/PKCE-only) |
| Created | Date |

**Actions:**

| Key | What it does |
|-----|--------------|
| `n` | Create client — prompts for name, redirect URIs, scopes, and type |
| `e` / `Enter` | Edit — modify name, redirect URIs, scopes, token TTLs |
| `d` | Delete client (permanent) |
| `r` | Manage roles assigned to this client (for M2M `client_credentials` flows) |

When creating a **confidential** client, the secret is shown exactly once after creation — copy it immediately.

---

### Users

All users within the selected tenant.

| Column | Notes |
|--------|-------|
| ID | Short UUID prefix |
| Email | Decrypted on-the-fly from the encrypted store |
| Status | `active` / `inactive` |
| MFA | ✓ if TOTP is enabled |
| Created | Date |

**Actions:**

| Key | What it does |
|-----|--------------|
| `n` | Create user — email + password |
| `e` / `Enter` | Open Edit User modal |
| `d` | Delete user (permanent) |

#### Edit User modal

The Edit User modal has **five sections**, cycled with `Tab`:

| Field index | Section | Controls |
|-------------|---------|----------|
| 0 | **Email** | Type to edit |
| 1 | **Password** | Type new password; leave blank to keep existing |
| 2 | **Status** | `Space` to toggle active/inactive |
| 3 | **Roles** | `↑/↓` to navigate, `Space` to assign/unassign |
| 4 | **Passkeys** | `↑/↓` to navigate, `d` to delete a passkey |

Press `Enter` to save email, password, status, and role assignments. Passkey deletion is immediate (no separate save needed).

The **Permissions** section (read-only) shows the permissions derived from currently assigned roles — it updates in real time as you toggle roles.

---

### Roles

Roles scoped to the selected tenant. Roles are assigned to users (in the Users modal) or to M2M clients (via `r` in the Clients tab).

| Key | Action |
|-----|--------|
| `n` | Create role — name + description |
| `e` | Edit role — modify name, description, and assigned permissions |
| `d` | Delete role |

---

### Permissions

Fine-grained permissions. Each permission is a `resource:action` pair (e.g., `documents:read`). Permissions are grouped into roles.

| Key | Action |
|-----|--------|
| `n` | Create permission — name + description |
| `e` | Edit permission — name + description |
| `d` | Delete permission |

---

### Sessions

Active sessions for the selected tenant.

| Column | Notes |
|--------|-------|
| User | Email of the session owner |
| IP | Client IP at login time |
| Created | Session creation time |
| Last seen | Most recent activity |
| Expires | Expiry timestamp |

Press `d` to revoke a session immediately. The associated refresh token is also invalidated.

---

### Settings

The Settings tab uses a **two-tier navigation model**:

1. **Tier 1** — `←/→` to move between section tabs
2. **Tier 2** — press `Enter` to enter a section; `Backspace` or `Esc` to exit back to Tier 1

Inside a section: `Tab` advances to the next field, `Space` toggles boolean fields, `Enter` saves and exits.

#### Password Policy

| Field | Type | Description |
|-------|------|-------------|
| Min Length | Number | Minimum password character count |
| Require Uppercase | Toggle | At least one A–Z character |
| Require Digit | Toggle | At least one 0–9 character |
| Require Special | Toggle | At least one `!@#$%^&*` character |

#### Lockout

| Field | Type | Description |
|-------|------|-------------|
| Max Attempts | Number | Failed logins before lockout |
| Window (minutes) | Number | Rolling window for counting attempts |
| Lockout Duration (minutes) | Number | How long the account stays locked |

#### Tokens

| Field | Type | Description |
|-------|------|-------------|
| Access Token TTL (minutes) | Number | Lifetime of issued access tokens |
| Refresh Token TTL (days) | Number | Lifetime of refresh tokens |

#### Registration

| Field | Type | Description |
|-------|------|-------------|
| Allow Public Registration | Toggle | Whether `POST /auth/register` is open to anyone |
| Require Email Verified to Login | Toggle | Block login until email is verified |

#### SMTP

Per-tenant outbound email configuration. Required for verification emails and password reset links.

| Field | Type | Description |
|-------|------|-------------|
| Host | Text | SMTP server hostname (e.g. `smtp.sendgrid.net`) |
| Port | Number | SMTP port (default: `587`) |
| Username | Text | SMTP auth username |
| Password | Masked | SMTP auth password — displayed as `•••`; leave blank on save to keep the existing value |
| From Name | Text | Sender display name |
| From Email | Text | Sender address |
| STARTTLS | Toggle | Upgrade connection with STARTTLS (recommended) |
| Enabled | Toggle | Master switch — no emails sent when disabled |

<Note>
  The password is stored AES-256-GCM encrypted with the tenant's key hierarchy and is never returned by the API.
</Note>

---

### IdP (Identity Providers)

Social login providers (Google, GitHub) configured for the selected tenant.

| Key | Action |
|-----|--------|
| `n` | Add provider — select type, enter client ID, secret, redirect URL, scopes |
| `e` | Edit provider |
| `d` | Delete provider |

Providers must be **Enabled** (toggle in the edit modal) to appear in the authorization flow.

---

### Audit Log

Read-only, append-only event log for the selected tenant. Events include logins, failures, MFA challenges, token issuances, password resets, WebAuthn authentication, and admin operations.

| Column | Description |
|--------|-------------|
| Action | Event type (e.g. `login.success`, `login.webauthn.success`) |
| User | User ID of the actor (if known) |
| IP | Client IP |
| Time | UTC timestamp |

Use `↑/↓` to scroll. Press `Enter` to see full event metadata.

---

## Quick Start wizard

Press `?` from the main screen to launch the Quick Start wizard. It walks through creating a tenant, an OAuth client, and a first user in three steps — useful for setting up a new environment from scratch.

---

## Tips

- **Tenant scope** — all tabs (users, clients, roles, sessions, settings) are scoped to the tenant currently highlighted in the sidebar. Switch tenants by pressing `↑/↓` in the sidebar.
- **Settings are saved per-section** — pressing `Enter` in any Settings sub-section saves only that section, not the entire settings page.
- **Passkey deletion is immediate** — pressing `d` in the Passkeys section of Edit User calls the delete API immediately without a confirmation prompt.
- **SMTP password** — leave the password field blank when saving SMTP settings to keep the existing encrypted value. The current value is never shown; instead you'll see `(configured — leave blank to keep)` if a password is already stored.
