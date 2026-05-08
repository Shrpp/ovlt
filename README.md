<div align="center">

<img src="https://cdn.ovlt.tech/logo.png" alt="OVLT logo" width="320" style="margin-bottom: 16px" />

**Auth infrastructure that fits in 20MB.**

OAuth2 + OIDC · Multi-tenant · Zero-knowledge encrypted · Self-hosted on your own terms.

<br/>

[![][badge-license]](LICENSE)
[![][badge-crate]](https://crates.io/crates/hefesto)
[![][badge-docker]](https://github.com/shrpp/ovlt/pkgs/container/ovlt-core)
[![][badge-status]](https://github.com/shrpp/ovlt/releases)
[![][badge-docs]](https://ovlt.tech/docs)
[![][badge-rust]](https://www.rust-lang.org)

[badge-license]: https://img.shields.io/badge/license-ELv2-00d4ff?style=flat-square&logoColor=white
[badge-crate]: https://img.shields.io/crates/v/hefesto?style=flat-square&label=hefesto&color=00d4ff&logo=rust&logoColor=white
[badge-docker]: https://img.shields.io/badge/docker-ghcr.io-00d4ff?style=flat-square&logo=docker&logoColor=white
[badge-status]: https://img.shields.io/badge/status-alpha-ff6b35?style=flat-square
[badge-docs]: https://img.shields.io/badge/docs-Mintlify-00d4ff?style=flat-square
[badge-rust]: https://img.shields.io/badge/built_with-Rust-f0ebe4?style=flat-square&logo=rust&logoColor=white

<p align="center">
  <img src="https://cdn.ovlt.tech/demo_tui.gif" alt="Demo OVLT" width="600" />
</p>

</div>

---

> [!WARNING]
> **Alpha build** — not production ready. APIs and configuration may change without notice.
> Do not use in production until a stable release is announced.

Keycloak needs a JVM and 512MB RAM. Authentik needs Redis and 735MB.  
OVLT runs in under 20MB — on the same $6 VPS your app already lives on.

Built with **Rust + Axum + PostgreSQL RLS**. Powered by [hefesto](https://crates.io/crates/hefesto).

---

## Quick Start

```bash
docker run -p 3000:3000 \
  -e OVLT_ADMIN_KEY=your-admin-key \
  -e OVLT_BOOTSTRAP_ADMIN_EMAIL=admin@example.com \
  -e OVLT_BOOTSTRAP_ADMIN_PASSWORD=Admin1234! \
  ghcr.io/shrpp/ovlt-core:latest
```

> Secrets (`JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY`) are **auto-generated** on first run and printed to logs. Save them somewhere safe.

---

## Features

| | |
|---|---|
| 🔐 **OIDC Authorization Server** | Authorization Code + PKCE, client_credentials (M2M), RS256 id_tokens, JWKS endpoint |
| 🏢 **Multi-tenant** | PostgreSQL RLS enforcement — tenant isolation at the database level, not the application level |
| 🔒 **Zero-knowledge encryption** | AES-256-GCM double-envelope at rest via [hefesto](https://crates.io/crates/hefesto) — the server never sees plaintext credentials |
| 📱 **MFA** | TOTP (RFC 6238) + WebAuthn/Passkeys (FIDO2 Level 2) — manage via TUI or API |
| 🌐 **Social login** | Google and GitHub OAuth2 — per-tenant IdP config stored in DB, manageable via TUI |
| 📧 **Per-tenant SMTP** | Encrypted credentials at rest · Auto-send on register + password reset |
| 📋 **Audit log** | Every auth event recorded — who, what, when, from where |
| 🖥️ **Admin TUI** | Terminal UI with guided wizard setup — manage tenants, users, clients, roles, SMTP, IdPs |
| 🔍 **OpenAPI + Swagger UI** | Auto-generated spec at `/openapi.json` · Interactive docs at `/docs` |
| 🛡️ **Security by default** | Argon2id passwords · Rotating refresh tokens · Account lockout · Per-IP rate limiting · HSTS · CSP |

---

## Comparison

| | OVLT | Keycloak | Authentik | Zitadel |
|:---|:---:|:---:|:---:|:---:|
| RAM at idle | **~20MB** | ~512MB | ~735MB | ~150MB |
| Startup time | **<1s** | 30–60s | ~10s | ~5s |
| Language | **Rust** | Java | Python | Go |
| Zero-knowledge enc. | ✅ | ❌ | ❌ | ❌ |
| Field-level encryption | ✅ | ❌ | ❌ | ❌ |
| Multi-tenant built-in | ✅ | ✅ | ✅ | ✅ |
| No external deps | ✅ | ❌ | ❌ (Redis) | ❌ |
| PKCE required | ✅ | Optional | Optional | Optional |
| Argon2id hashing | ✅ | ❌ (bcrypt) | ✅ | ✅ |
| Runs on $6 VPS | ✅ | ❌ | ❌ | ⚠️ |
| Pricing | **Free** | Free | Free | Free |

---

## Install Admin TUI

Download the `ovlt` binary from [GitHub Releases](https://github.com/shrpp/ovlt/releases/latest) for your platform.  
Binaries are named `ovlt-<platform>-<version>` (e.g. `ovlt-macos-arm64-v0.1.0`).

**macOS**
```bash
# Remove quarantine flag (required — binary is unsigned in alpha)
xattr -dr com.apple.quarantine ovlt-macos-arm64-*   # M1/M2/M3/M4
# or
xattr -dr com.apple.quarantine ovlt-macos-x64-*     # Intel

chmod +x ovlt-macos-*
sudo mv ovlt-macos-* /usr/local/bin/ovlt
ovlt connect http://localhost:3000
```

**Linux** (static musl — zero dependencies)
```bash
chmod +x ovlt-linux-x64-*    # x86_64
# or
chmod +x ovlt-linux-arm64-*  # ARM64

sudo mv ovlt-linux-* /usr/local/bin/ovlt
ovlt connect http://localhost:3000
```

**Windows**
```powershell
Move-Item ovlt-windows-x64-*.exe ovlt.exe
.\ovlt.exe connect http://localhost:3000
```
> Windows SmartScreen will show a warning because the binary is not yet code-signed. Click **More info → Run anyway** to proceed.

Once connected, launch the TUI anytime with just `ovlt serve`. It guides you through tenant creation, user management, client registration, SMTP config, and more — no web browser required.

> **Alpha notice:** Homebrew tap, WinGet/Scoop, and code signing are planned for the stable beta release.

---

## Documentation

| Doc | Description |
|:----|:------------|
| [Getting Started](docs/getting-started.md) | Run OVLT, first login, create a tenant |
| [Configuration](docs/configuration.md) | All environment variables |
| [API Reference](docs/api-reference.md) | All HTTP endpoints · Interactive Swagger UI at `/docs` |
| [Admin TUI](docs/admin-tui.md) | Using the `ovlt` terminal UI |
| [M2M / Client Credentials](docs/m2m.md) | Machine-to-machine auth flow |
| [MFA](docs/mfa.md) | TOTP setup and management |
| [WebAuthn / Passkeys](docs/webauthn.md) | FIDO2 passkey registration and authentication |
| [SMTP](docs/smtp.md) | Per-tenant email delivery configuration |
| [Social Login](docs/social-login.md) | Google and GitHub OAuth setup |
| [Architecture](docs/architecture.md) | Multi-tenancy, RLS, encryption model |
| [Security](docs/security.md) | Security model, threat model, hardening |
| [Database Access](docs/database.md) | Connect via DataGrip/DBeaver locally or over SSH tunnel |

---

## Technology Stack

| Layer | Technology | Why |
|:------|:-----------|:----|
| Runtime | Rust | Memory-safe, no garbage collector, zero-cost abstractions |
| Web framework | Axum | Async, composable, built on Tokio |
| Database | PostgreSQL + RLS | Tenant isolation enforced at the database level |
| ORM | SeaORM | Type-safe queries, automatic migrations on startup |
| Encryption | [hefesto](https://crates.io/crates/hefesto) | AES-256-GCM, double-envelope key wrapping, zero-knowledge design |
| Hashing | Argon2id | Current recommended standard for password hashing |
| Protocols | OAuth2, OIDC, JWT | RS256 id_tokens, HS256 access tokens, JWKS endpoint |
| Deployment | Docker + Compose | Single binary, no sidecars, no external dependencies except PostgreSQL |

---

## Roadmap

```
   ✓  Stage 1 · Auth Core                                               [ done ]
   │    OIDC compliance — authorization_code + PKCE, client_credentials
   │    RS256 id_token, JWKS, OpenID discovery
   │    Refresh token rotation, revocation, session management
   │    RBAC — roles, permissions, claims in tokens
   │
   ✓  Stage 2 · User Lifecycle                                          [ done ]
   │    Password reset + email verification (one-time tokens)
   │    Password policies, account lockout, login attempt tracking
   │    Per-tenant settings (TTL, lockout thresholds, registration toggle)
   │
   ✓  Stage 3 · Extended Auth                                           [ done ]
   │    TOTP MFA (RFC 6238) — setup, challenge, disable, admin override
   │    WebAuthn / Passkeys (FIDO2 Level 2) — register, authenticate, manage
   │    Social login — Google + GitHub, per-tenant IdP config via DB + TUI
   │    Per-tenant SMTP — encrypted credentials, auto-send on register + reset
   │
   ✓  Stage 4 · DX & Observability                                      [ done ]
   │    OpenAPI spec — GET /openapi.json + Swagger UI at GET /docs
   │    Universal login — no tenant picker, server resolves from email
   │    Structured errors — field-level validation, friendly messages
   │    TUI redesign — consistent keybindings, contextual hints, settings modals
   │    CLI — ovlt serve / ovlt connect, cross-platform static binaries
   │    Audit log, database access docs
   │
   ●  Stage 5 · Security Hardening                               [ in progress ]
   │    Cookie Secure flag in production
   │    Remove raw SQL string interpolation (defense-in-depth)
   │    Type-safe RLS extractor — impossible to bypass at compile time
   │    Distributed rate limiter (PostgreSQL-backed, multi-replica safe)
   │    MFA backup codes — recovery without admin intervention
   │    Tenant key cache with TTL + zeroize-on-drop
   │    Key rotation with grace period (JWT + RSA)
   │    Docker image hardening (distroless, non-root)
   │    Integration test suite (authorize → token → introspect)
   │    Password history enforcement
   │    Durable audit log (critical events synchronous, buffered otherwise)
   │
   ○  Stage 6 · Observability & Developer Experience              [ planned ]
   │    Prometheus metrics — /metrics endpoint (login rates, token issuance,
   │      DB pool, JWKS cache hits)
   │    Structured tracing — request-id correlation end-to-end
   │    JS SDK (@ovlt/sdk) — PKCE flow, auto-refresh, TS types  [ under evaluation ]
   │    Auto-generated SDKs — Python + Go via OpenAPI Generator on release
   │    Framework integration guides — Next.js, Express, FastAPI, SvelteKit
   │    Single-tenant convenience mode (DEFAULT_TENANT_SLUG env var)
   │    Dev-mode rich errors — debug field with human-readable context
   │    Homebrew tap · WinGet/Scoop · Code-signed binaries
   │
   ○  Stage 7 · Beta Features                                     [ planned ]
   │    Generic OIDC identity broker — Microsoft, Apple, Okta, any OIDC IdP
   │      via discovery, not just Google + GitHub
   │    User groups — hierarchical, role inheritance, JWT claims
   │    Tenant export / import — disaster recovery, staging→prod migration
   │    Token introspection with bloom filter cache
   │    Bulk user import — NDJSON, supports Argon2id passthrough + re-hash
   │    Account self-service — sessions list/revoke, email change, password
   │      change with current-password verification
   │
   ○  Stage 8 · Distribution & Auto-update                        [ planned ]
   │    Auto-update check on startup — pulls latest binary if newer version available
   │    In-place binary replacement without reinstall or service interruption
   │    Configurable update channel (stable / beta) via env var or config file
   │    Opt-out flag for air-gapped / locked environments
   │
   ◉  Stable beta — Q3 2026
```

> Have a feature in mind or found a bug? [Open a Discussion →](https://github.com/shrpp/ovlt/discussions)

---

## License

[Elastic License 2.0](LICENSE) — free to self-host and contribute.  
Cannot be resold or offered as a managed service by third parties.  
This protects the project's long-term sustainability while keeping the source open.

---

> [!NOTE]
> **Built with Self-Driven Development (SDD)** — AI is used to accelerate development
> velocity on boilerplate, documentation, and iteration cycles. All architecture decisions,
> security design, and code review are done by the author. Contributions and audits welcome.

---

<div align="center">

[ovlt.tech](https://ovlt.tech) · [me@shrpp.dev](mailto:me@shrpp.dev) · powered by [hefesto](https://crates.io/crates/hefesto)

</div>
