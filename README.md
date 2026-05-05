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

</div>

---

> [!WARNING]
> **Alpha build** — not production ready. APIs and configuration may change without notice.
> Do not use in production until a stable release is announced.

Keycloak needs a JVM and 512MB RAM. Authentik needs Redis and 735MB.  
OVTL runs in under 20MB — on the same $6 VPS your app already lives on.

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
| 📱 **MFA** | TOTP support for authenticator apps (Authy, Google Authenticator, etc.) |
| 🌐 **Social login** | Google and GitHub OAuth2 out of the box |
| 📋 **Audit log** | Every auth event recorded — who, what, when, from where |
| 🖥️ **Admin TUI** | Terminal UI to manage tenants, users, clients, roles, and permissions with guided wizard setup |
| 🛡️ **Security by default** | Argon2id passwords · Rotating refresh tokens · Account lockout · Per-IP rate limiting · HSTS · CSP |

---

## Comparison

| | OVTL | Keycloak | Authentik | Zitadel |
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

**macOS**
```bash
# Remove quarantine flag (required — binary is unsigned in alpha)
xattr -dr com.apple.quarantine ovlt-aarch64-apple-darwin   # M1/M2/M3
# or
xattr -dr com.apple.quarantine ovlt-x86_64-apple-darwin    # Intel

chmod +x ovlt-*-apple-darwin
sudo mv ovlt-*-apple-darwin /usr/local/bin/ovlt
ovlt --url http://localhost:3000
```

**Linux**
```bash
chmod +x ovlt-x86_64-unknown-linux-gnu   # x86_64
# or
chmod +x ovlt-aarch64-unknown-linux-gnu  # ARM64

sudo mv ovlt-*-linux-* /usr/local/bin/ovlt
ovlt --url http://localhost:3000
```

**Windows**
```powershell
# Rename and move to a folder in your PATH
Move-Item ovlt-windows-x86_64.exe ovlt.exe

# Run directly
.\ovlt.exe --url http://localhost:3000
```
> Windows SmartScreen will show a warning because the binary is not yet code-signed. This is expected for alpha builds. Click **More info → Run anyway** to proceed.

The TUI guides you through tenant creation, user management, client registration, and permissions — no web browser required.

> **Alpha notice:** Installation steps above are intentionally manual. Homebrew tap (macOS), WinGet/Scoop (Windows), and code signing are planned for the stable beta release. This is subject to change.

---

## Documentation

| Doc | Description |
|:----|:------------|
| [Getting Started](docs/getting-started.md) | Run OVTL, first login, create a tenant |
| [Configuration](docs/configuration.md) | All environment variables |
| [API Reference](docs/api-reference.md) | All HTTP endpoints |
| [Admin TUI](docs/admin-tui.md) | Using the `ovlt` terminal UI |
| [M2M / Client Credentials](docs/m2m.md) | Machine-to-machine auth flow |
| [MFA](docs/mfa.md) | TOTP setup and management |
| [Architecture](docs/architecture.md) | Multi-tenancy, RLS, encryption model |
| [Security](docs/security.md) | Security model, threat model, hardening |

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

The path to a stable beta is divided into focused stages — each one must be complete before the next begins.

```
  now
   │
   ●  Stage 1 · OIDC Compliance                                     [ in progress ]
   │    refresh_token grant at /oauth/token
   │    /oauth/userinfo endpoint
   │    RS256-signed access tokens (JWKS-verifiable)
   │    Roles in introspect response (realm_access / resource_access)
   │
   ●  Stage 2 · Email Delivery                                       [ pending ]
   │    SMTP integration via lettre (pluggable config)
   │    Password reset emails
   │    Email verification on register
   │
   ●  Stage 3 · TUI Completeness                                     [ pending ]
   │    Settings tab fully wired (policy, TTL, lockout)
   │    Identity Providers tab wired (per-tenant social login via DB)
   │
   ●  Stage 4 · Production Hardening                                 [ pending ]
   │    Semver releases + CHANGELOG (v0.1.0)
   │    Docker image hardening (distroless, non-root)
   │    Rate limit thresholds documented and configurable
   │
   ●  Stage 5 · Extended Features                                    [ pending ]
   │    HTTP integration tests (authorize → token → introspect)
   │    Webhook events (login, logout, MFA)
   │    WebAuthn / Passkeys
   │    DB-stored social IDPs (per-tenant, configurable via TUI)
   │
   ◉  Early access — Q3 2026
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
