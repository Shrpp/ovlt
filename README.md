<p align="center">
  <img src="https://cdn.ovlt.tech/logo.png" alt="OVLT logo" />
</p>

<p align="center">
  <strong>Self-hosted OIDC auth in a single Rust binary.</strong><br/>
  OAuth2 + OIDC · Multi-tenant via Postgres RLS · <50 MB RAM · MIT licensed.
</p>

<p align="center">
  <a href="https://github.com/Shrpp/ovlt/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-00d4ff?style=flat-square&logoColor=white" alt="MIT License"/></a>
  <a href="https://crates.io/crates/hefesto"><img src="https://img.shields.io/crates/v/hefesto?style=flat-square&label=hefesto&color=00d4ff&logo=rust&logoColor=white" alt="hefesto on crates.io"/></a>
  <a href="https://github.com/shrpp/ovlt/pkgs/container/ovlt-core"><img src="https://img.shields.io/badge/docker-ghcr.io-00d4ff?style=flat-square&logo=docker&logoColor=white" alt="Docker image"/></a>
  <a href="https://github.com/shrpp/ovlt/releases"><img src="https://img.shields.io/badge/status-alpha-ff6b35?style=flat-square" alt="Alpha status"/></a>
  <a href="https://ovlt.tech/docs"><img src="https://img.shields.io/badge/docs-Mintlify-00d4ff?style=flat-square" alt="Documentation"/></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/built_with-Rust-f0ebe4?style=flat-square&logo=rust&logoColor=white" alt="Built with Rust"/></a>
</p>

<p align="center">
  <img src="https://cdn.ovlt.tech/demo_tui.gif" alt="OVLT TUI demo" />
</p>

---

> **⚠ Alpha build.** Not production ready. APIs and configuration may change.
> Stable beta target: Q3 2026. See [Roadmap](#roadmap) and [Security](docs/security.md)
> for current state and known gaps.

OVLT is a self-hosted OIDC authorization server in Rust, designed for indie hackers and small teams who want OIDC without the operational overhead of Keycloak or the per-seat cost of hosted alternatives. Single binary, Postgres-only, no sidecars.

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

> Secrets (`JWT_SECRET`, `MASTER_ENCRYPTION_KEY`, `TENANT_WRAP_KEY`) are **auto-generated** on first run and printed to logs. Save them somewhere safe before restarting the container.

Once running, see [Getting Started](docs/getting-started.md) to create your first tenant and integrate a client application.

---

## Features

|  |  |
| --- | --- |
| 🔐 **OIDC Authorization Server** | Authorization Code + PKCE, client_credentials (M2M), RS256 id_tokens, JWKS endpoint, OpenID discovery |
| 🏢 **Multi-tenant isolation** | PostgreSQL Row-Level Security — tenant boundaries enforced at the database layer, not the application layer |
| 🔒 **Encryption at rest** | AES-256-GCM envelope encryption for sensitive fields (TOTP secrets, SMTP credentials, IdP secrets) via [hefesto](https://crates.io/crates/hefesto) |
| 📱 **MFA** | TOTP (RFC 6238) + WebAuthn/Passkeys (FIDO2 Level 2) — manage via TUI or API |
| 🌐 **Social login** | Google and GitHub OAuth2 — per-tenant IdP config, stored encrypted, manageable via TUI |
| 📧 **Per-tenant SMTP** | Encrypted credentials at rest · Auto-send on register + password reset |
| 📋 **Audit log** | Every auth event recorded — who, what, when, from where |
| 🖥️ **Admin TUI** | Terminal UI with guided wizard — manage tenants, users, clients, roles, SMTP, IdPs |
| 🔍 **OpenAPI + Swagger UI** | Auto-generated spec at `/openapi.json` · Interactive docs at `/docs` |
| 🛡️ **Hardened defaults** | Argon2id passwords · Rotating refresh tokens · Account lockout · Per-IP rate limiting · HSTS · CSP |

### Planned for v0.5

- **OPAQUE password authentication** — the server never sees the user's password, even during login. Real zero-knowledge for credentials. [Tracking issue](#)
- **Client-side encrypted tenant secrets** — SMTP passwords and IdP secrets encrypted in the admin's browser before transmission. Server holds only ciphertext at rest. [Tracking issue](#)

---

## Comparison

|  | OVLT | Kanidm | Keycloak | Authentik | Zitadel |
| --- | --- | --- | --- | --- | --- |
| RAM at idle | **<50 MB** | ~90 MB | ~512 MB | ~735 MB | ~512 MB¹ |
| Startup time | **<1 s** | <2 s | 30–60 s | ~10 s | ~5 s |
| Language | **Rust** | Rust | Java | Python | Go |
| Multi-tenant built-in | ✅ | ✅ | ✅ | ✅ | ✅ |
| Field-level encryption | ✅ | ❌ | ❌ | ❌ | ❌ |
| External deps beyond DB | None | None | None | Redis | None |
| PKCE required | ✅ | ✅ | Optional | Optional | Optional |
| Argon2id hashing | ✅ | ✅ | ❌ (bcrypt default) | ✅ | ✅ |
| OPAQUE auth | 🚧 v0.5 | ❌ | ❌ | ❌ | ❌ |
| Maturity | Alpha | Stable | Stable | Stable | Stable |
| License | MIT | MPL-2.0 | Apache 2.0 | MIT | Apache 2.0 |
| Pricing | Free | Free | Free | Free | Free² |

<sup>¹ Per the Zitadel maintainer's recommendation for small setups. Lower minimums are technically possible.</sup>
<br/>
<sup>² Zitadel Cloud is paid; self-hosted is free.</sup>

> **Measurement methodology:** RAM at idle measured with 1 tenant, 10 users, no active sessions, fresh container. Steady-state under load may differ.

This is not a "we beat them at everything" table. The alternatives above are all mature, production-tested projects with track records OVLT does not yet have. OVLT's bet is on a different operational profile (single Rust binary, no JVM/Redis, low footprint, modern cryptography roadmap), not on being feature-complete relative to Keycloak.

---

## Install Admin TUI

Download the `ovlt` binary from [GitHub Releases](https://github.com/shrpp/ovlt/releases/latest). Binaries are named `ovlt-<platform>-<version>`.

**macOS**

```bash
# Remove quarantine flag (required — binary is unsigned in alpha)
xattr -dr com.apple.quarantine ovlt-macos-arm64-*   # Apple Silicon
# or
xattr -dr com.apple.quarantine ovlt-macos-x64-*     # Intel

chmod +x ovlt-macos-*
sudo mv ovlt-macos-* /usr/local/bin/ovlt
ovlt connect http://localhost:3000
```

**Linux** (static musl, zero runtime dependencies)

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

> Windows SmartScreen will warn — the binary is not yet code-signed. Click **More info → Run anyway**. Homebrew tap, WinGet/Scoop, and code-signed binaries are planned for the stable beta.

Once connected, run `ovlt serve` to launch the TUI. It guides you through tenant creation, user management, client registration, SMTP setup, and more — no web browser required.

---

## Documentation

| Doc | Description |
| --- | --- |
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
| [Security](docs/security.md) | Security model, threat model, hardening checklist |
| [Database Access](docs/database.md) | Connect via DataGrip/DBeaver locally or over SSH tunnel |

---

## Technology Stack

| Layer | Technology | Why |
| --- | --- | --- |
| Runtime | Rust | Memory-safe, no garbage collector, predictable footprint |
| Web framework | Axum | Async, composable, built on Tokio |
| Database | PostgreSQL + RLS | Tenant isolation enforced at the database layer |
| ORM | SeaORM | Type-safe queries, automatic migrations on startup |
| Encryption | [hefesto](https://crates.io/crates/hefesto) | AES-256-GCM envelope encryption (see [Security](docs/security.md) for the key hierarchy) |
| Password hashing | Argon2id | Current OWASP recommendation (OPAQUE migration planned for v0.5) |
| Protocols | OAuth2, OIDC, JWT | RS256 id_tokens, HS256 access tokens, JWKS endpoint |
| Deployment | Docker + Compose | Single binary, no sidecars, no external dependencies except Postgres |

---

## Maturity

OVLT is alpha software. Where the project currently stands:

- **Implemented and exercised:** OIDC core flows, multi-tenant isolation, MFA, social login, SMTP, audit log, TUI.
- **Implemented but lightly tested:** Encryption at rest (hefesto), key rotation, advanced rate limiting.
- **In progress:** Comprehensive integration test suite (Stage 5), distributed rate limiter, OPAQUE password auth (Stage 6).
- **Not yet started:** External security audit, hardened production deployment guide, multi-region replication.

The [Roadmap](#roadmap) reflects this honestly. If you're evaluating OVLT for production use, the answer is "not yet" — wait for the stable beta in Q3 2026. If you're evaluating it for a side project, internal tool, or test deployment, the alpha works and feedback is welcome.

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
│    Universal login — server resolves tenant from email
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
│    Tenant key cache with TTL + zeroize-on-drop
│    Key rotation with grace period (JWT + RSA)
│    Docker image hardening (distroless, non-root)
│    Password history enforcement
│    Durable audit log (critical events synchronous, buffered otherwise)
│    MFA backup codes — recovery without admin intervention
│
●  Stage 6 · Test Suite Expansion                             [ in progress ]
│    Integration test suite — full OIDC flow (authorize → token → introspect)
│    Tenant isolation regression tests across every endpoint
│    Security regression tests — enumeration, timing, tampering
│    Cryptographic round-trip tests for hefesto envelope
│    OAuth flow tests — PKCE enforcement, code replay, JWT tamper rejection
│    Coverage gates in CI (block PRs that drop coverage > 2%)
│    Target: ≥60% overall, ≥80% in auth/, ≥90% in crypto/
│    Public coverage badge and per-module reporting
│
○  Stage 7 · Modern Authentication                             [ planned ]
│    OPAQUE password authentication (via opaque-ke)
│      — server never sees the password, even during login
│      — migration path from Argon2id for existing users
│    Client-side envelope encryption for tenant configuration secrets
│      — admin browser encrypts SMTP password and IdP secrets before transmit
│      — server holds only ciphertext at rest
│    JS client SDK (@ovlt/sdk) implementing OPAQUE on the browser side
│    Documented threat model in docs/security.md
│
○  Stage 8 · Observability & Developer Experience              [ planned ]
│    Prometheus metrics — /metrics endpoint (login rates, token issuance,
│      DB pool, JWKS cache hits)
│    Structured tracing — request-id correlation end-to-end
│    Auto-generated SDKs — Python + Go via OpenAPI Generator on release
│    Framework integration guides — Next.js, Express, FastAPI, SvelteKit
│    Single-tenant convenience mode (DEFAULT_TENANT_SLUG env var)
│    Dev-mode rich errors — debug field with human-readable context
│    Homebrew tap · WinGet/Scoop · Code-signed binaries
│
○  Stage 9 · Beta Features                                     [ planned ]
│    Generic OIDC identity broker — Microsoft, Apple, Okta, any OIDC IdP
│      via discovery, not just Google + GitHub
│    User groups — hierarchical, role inheritance, JWT claims
│    Tenant export / import — disaster recovery, staging→prod migration
│    Token introspection with bloom filter cache
│    Bulk user import — NDJSON, supports Argon2id passthrough + re-hash
│    Account self-service — sessions list/revoke, email change, password
│      change with current-password verification
│
○  Stage 10 · Distribution & Auto-update                       [ planned ]
│    Auto-update check on startup — pulls latest binary if newer available
│    In-place binary replacement without reinstall or service interruption
│    Configurable update channel (stable / beta) via env var or config file
│    Opt-out flag for air-gapped / locked environments
│
◉  Stable beta — Q3 2026
```

> Have a feature in mind or found a bug? [Open a Discussion](https://github.com/shrpp/ovlt/discussions). Found a security issue? See [Security](docs/security.md) for the reporting process.

---

## How to help

OVLT is a one-maintainer project. The highest-impact contributions right now:

1. **Security review.** Read the code. File issues for anything that looks off. Especially: auth endpoints, token validation, RLS extractor, encryption boundaries. See the [Security policy](docs/security.md) for how to report.
2. **Tests.** Stage 6 is actively in progress. Integration tests, regression tests for fixed bugs, and tenant isolation tests are all open for contribution. See [CONTRIBUTING.md](CONTRIBUTING.md).
3. **Production smoke-testing.** Deploy OVLT in a staging environment. Report what breaks. Bugs found in real deployments are worth ten times bugs found in code review.
4. **Documentation.** If something confused you during setup, that's a doc bug. Doc PRs are reviewed fast.
5. **Client SDKs.** OPAQUE will need a JS client. Python and Go SDKs are planned. Early contributors welcome.

For larger work (new auth flow, encryption change, multi-tenancy logic), open an issue first to align.

---

## License

[MIT License](LICENSE) — free to use, modify, and distribute.

---

## How this project is built

AI tools (Claude, GitHub Copilot) accelerate boilerplate, docs, and iteration cycles. Every commit is reviewed by the author. Architecture and security decisions are the author's responsibility — and the gaps in tests, threat model, and external review are also the author's responsibility to close. The Roadmap reflects current state honestly. Code review, security reports, and contributions are welcome and credited.

---

<p align="center">
  <a href="https://ovlt.tech">ovlt.tech</a> · <a href="mailto:me@shrpp.dev">me@shrpp.dev</a> · powered by <a href="https://crates.io/crates/hefesto">hefesto</a>
</p>
