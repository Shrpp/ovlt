---
title: Getting Started
description: Run OVLT in under 5 minutes.
---

<Steps>
  <Step title="Start the server">
    The fastest way is Docker. On first run, OVLT auto-generates the three cryptographic secrets and prints them to logs.

    ```bash
    docker run -d --name ovlt -p 3000:3000 \
      -e DATABASE_URL=postgresql://user:pass@host:5432/ovlt \
      -e OVLT_ADMIN_KEY=your-admin-key \
      -e OVLT_BOOTSTRAP_ADMIN_EMAIL=admin@example.com \
      -e OVLT_BOOTSTRAP_ADMIN_PASSWORD=Admin1234! \
      ghcr.io/shrpp/ovlt-core:latest
    ```

    Check the logs immediately:

    ```bash
    docker logs ovlt 2>&1 | grep -A5 "SECRETS GENERATED"
    ```

    You'll see:

    ```
    ╔══════════════════════════════════════════════════════╗
    ║           OVLT — FIRST RUN: SECRETS GENERATED       ║
    ║  JWT_SECRET=<base64>                                 ║
    ║  MASTER_ENCRYPTION_KEY=<base64>                      ║
    ║  TENANT_WRAP_KEY=<base64>                            ║
    ╚══════════════════════════════════════════════════════╝
    ```

    <Warning>
      Save these three values immediately. Losing them makes all encrypted data permanently unrecoverable.
    </Warning>
  </Step>

  <Step title="Pin your secrets and restart">
    Stop the container and re-run with the secrets pinned:

    ```bash
    docker stop ovlt && docker rm ovlt

    docker run -d --name ovlt -p 3000:3000 \
      -e DATABASE_URL=postgresql://user:pass@host:5432/ovlt \
      -e JWT_SECRET=<value-from-logs> \
      -e MASTER_ENCRYPTION_KEY=<value-from-logs> \
      -e TENANT_WRAP_KEY=<value-from-logs> \
      -e OVLT_ADMIN_KEY=your-admin-key \
      -e OVLT_BOOTSTRAP_ADMIN_EMAIL=admin@example.com \
      -e OVLT_BOOTSTRAP_ADMIN_PASSWORD=Admin1234! \
      ghcr.io/shrpp/ovlt-core:latest
    ```

    Verify it's healthy:

    ```bash
    curl http://localhost:3000/health
    # {"status":"ok","version":"x.y.z"}
    ```
  </Step>

  <Step title="Docker Compose (recommended for local dev)">
    Use this `docker-compose.yml` to get Postgres + OVLT in one command:

    ```yaml
    version: "3.9"
    services:
      postgres:
        image: postgres:16
        environment:
          POSTGRES_USER: ovlt
          POSTGRES_PASSWORD: ovlt
          POSTGRES_DB: ovlt
        volumes:
          - pg_data:/var/lib/postgresql/data

      ovlt:
        image: ghcr.io/shrpp/ovlt-core:latest
        ports:
          - "3000:3000"
        environment:
          DATABASE_URL: postgresql://ovlt:ovlt@postgres:5432/ovlt
          OVLT_ADMIN_KEY: change-me
          OVLT_BOOTSTRAP_ADMIN_EMAIL: admin@example.com
          OVLT_BOOTSTRAP_ADMIN_PASSWORD: Admin1234!
          # Paste generated secrets here after first run:
          # JWT_SECRET:
          # MASTER_ENCRYPTION_KEY:
          # TENANT_WRAP_KEY:
        depends_on:
          postgres:
            condition: service_started

    volumes:
      pg_data:
    ```

    ```bash
    docker compose up -d
    docker compose logs ovlt   # grab the generated secrets
    ```
  </Step>

  <Step title="Install the Admin TUI">
    The `ovlt` binary is a terminal UI to manage tenants, users, clients, roles, and permissions.

    <Tabs>
      <Tab title="macOS (Apple Silicon)">
        ```bash
        curl -Lo ovlt https://github.com/shrpp/ovlt/releases/latest/download/ovlt-aarch64-apple-darwin
        xattr -dr com.apple.quarantine ovlt
        chmod +x ovlt && sudo mv ovlt /usr/local/bin/
        ```

        <Note>
          The `xattr` command is required because the binary is unsigned in alpha. macOS Gatekeeper blocks unsigned binaries by default.
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
  </Step>

  <Step title="Connect and log in">
    ```bash
    ovlt --url http://localhost:3000
    # or: OVLT_URL=http://localhost:3000 ovlt
    ```

    When prompted for the **Admin Key**, enter the value you set in `OVLT_ADMIN_KEY`.

    A **master** tenant is created automatically on first startup using your bootstrap credentials. Navigate with arrow keys or `j`/`k`. Press `?` for the full key reference.
  </Step>
</Steps>

## Next steps

<CardGroup cols={2}>
  <Card title="Configuration" icon="sliders" href="/docs/configuration">
    All environment variables and production checklist
  </Card>
  <Card title="Admin TUI" icon="terminal" href="/docs/admin-tui">
    Full keyboard reference and tab guide
  </Card>
  <Card title="M2M / Client Credentials" icon="server" href="/docs/m2m">
    Service-to-service auth with embedded roles
  </Card>
  <Card title="API Reference" icon="rectangle-terminal" href="/docs/api-reference">
    All HTTP endpoints
  </Card>
</CardGroup>
