---
title: Admin TUI
description: Install and use the ovlt terminal UI to manage tenants, users, clients, roles, and permissions.
---

The `ovlt` binary is a terminal UI that connects to a running OVLT server over HTTP. Everything in the TUI maps 1:1 to an API call — use the [API Reference](/docs/api-reference) for scripted automation.

## Install

<Tabs>
  <Tab title="macOS (Apple Silicon)">
    ```bash
    curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-aarch64-apple-darwin
    xattr -dr com.apple.quarantine ovlt
    chmod +x ovlt && sudo mv ovlt /usr/local/bin/
    ```

    <Note>
      `xattr -dr com.apple.quarantine` is required because the binary is unsigned in alpha. macOS Gatekeeper blocks it without this step.
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

    <Note>
      Windows SmartScreen will warn about the unsigned binary. Click **More info → Run anyway**.
    </Note>
  </Tab>
</Tabs>

## Connect

```bash
ovlt --url http://localhost:3000
# or via env var:
OVLT_URL=http://localhost:3000 ovlt
```

On launch you are prompted for the **Admin Key** — the value you set in `OVLT_ADMIN_KEY` on the server.

## Keyboard reference

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Move between tabs |
| `↑` / `↓` or `j` / `k` | Move selection up/down |
| `Enter` | Open item / confirm action |
| `Esc` | Close modal / cancel |
| `n` | New item |
| `d` | Delete selected |
| `e` | Edit selected |
| `r` | Manage roles (clients only) |
| `?` | Toggle help overlay |
| `q` | Quit |

## Tabs

<AccordionGroup>
  <Accordion title="Tenants">
    List, create, and manage tenants. Each tenant is fully isolated — its users, clients, roles, and sessions belong to it alone.

    - Press `n` to create a new tenant
    - Select a tenant to scope all other tabs to it
  </Accordion>

  <Accordion title="Users">
    All users within the selected tenant.

    - Create, edit, and delete users
    - Reset passwords (generates a one-time reset token)
    - Get email verification codes
    - Admin-disable MFA for locked-out users
  </Accordion>

  <Accordion title="Clients">
    OAuth 2.0 clients registered within the selected tenant.

    | Field | Notes |
    |-------|-------|
    | Name | Display name |
    | Client ID | Auto-generated on creation |
    | Client Secret | Auto-generated; shown once — save immediately |
    | Grant Types | `authorization_code`, `client_credentials`, or both |
    | Redirect URIs | Required for `authorization_code` flows |
    | Scopes | Space-separated list of allowed scopes |

    For M2M (`client_credentials`) clients, press `r` to assign roles that will be embedded in issued tokens.
  </Accordion>

  <Accordion title="Roles">
    Roles scoped to the selected tenant. Roles can be assigned to users or to M2M clients.
  </Accordion>

  <Accordion title="Permissions">
    Fine-grained permissions (resource + action pairs). Permissions are assigned to roles, which are then assigned to users or clients.
  </Accordion>

  <Accordion title="Sessions">
    Active sessions for the tenant. Press `d` to revoke a session immediately.
  </Accordion>

  <Accordion title="Audit Log">
    Read-only view of all auth events — logins, logouts, failures, MFA events, token issuances — for the selected tenant.
  </Accordion>
</AccordionGroup>
