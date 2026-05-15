# Contributing to OVLT

OVLT is a small project with big ambitions — a developer-first OIDC auth service that just works, without the Keycloak operational tax or the Auth0 price tag. Every contribution moves it forward.

Licensed under the [MIT License](LICENSE) — use it, modify it, redistribute it, build a business on top of it. No restrictions beyond keeping the copyright notice.

---

## Where to start

Not sure what to work on? Here are the highest-impact contributions right now, in rough order of leverage:

### 1. Security review

OVLT is alpha and unaudited. Read the code. File issues for anything that looks off. Especially valuable:

- Auth endpoint flows in `ovlt-core/src/auth/` — anything that returns data before authentication completes is suspect
- Token validation logic — JTI handling, refresh token rotation, JWT signature verification
- The RLS extractor and any DB query that bypasses it
- Encryption boundaries — where plaintext crosses into ciphertext and back
- `hefesto` crate — the encryption layer that backs all sensitive fields at rest

**Found a vulnerability?** Do not open a public issue. Email `me@shrpp.dev` with subject `SECURITY: <description>`. See the [Security policy](docs/security.md) for response commitments.

### 2. Tests

Test coverage is actively being expanded (Stage 6 of the roadmap). Open contribution areas:

- **Integration tests** — full OIDC flows (authorize → token → introspect, refresh, revoke)
- **Tenant isolation regression tests** — for each endpoint, verify a token from tenant A cannot read/write data of tenant B
- **Security regression tests** — enumeration, timing, token tampering, JWT alg confusion, replay attacks
- **Crypto round-trips** — encrypt/decrypt, tamper rejection, key rotation
- **PKCE enforcement** — ensure the token endpoint rejects `code` without `code_verifier`

Adding a regression test for any historical bug is always welcome.

### 3. Production smoke-testing

Deploy OVLT in a staging environment. Run real traffic through it. Report what breaks. Real-world bug reports are worth 10x more than synthetic ones. Useful details to include:

- Deployment shape (single VPS, Kubernetes, fly.io, etc.)
- PostgreSQL version
- Approximate request volume
- Specific feature exercised (OIDC code flow, M2M, MFA, passkeys, SMTP)
- What you saw vs. what you expected

### 4. Documentation

If you spent more than 15 minutes stuck on something during setup, that's a docs bug worth filing. Direct doc PRs are reviewed fast and almost always merged.

### 5. Client SDKs

OPAQUE (Stage 7) will need a JS client. Python and Go SDKs are also planned. Early SDK contributors get co-authorship on the resulting packages.

### 6. Smaller good first issues

Look for the `good first issue` label on GitHub. These are scoped, self-contained, and well-documented.

For larger work (new auth flow, encryption change, multi-tenancy logic), **open an issue first** to align before building. Saves everyone time.

---

## Development setup

```bash
git clone https://github.com/shrpp/ovlt
cd ovlt
cp ovlt-core/.env.example ovlt-core/.env

# Start PostgreSQL
docker compose up -d postgres

# Run the server (migrations run automatically on startup)
cd ovlt-core
cargo run
```

Run tests:

```bash
cargo test
```

Clippy (CI enforces this — fix before opening a PR):

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Format:

```bash
cargo fmt
```

Coverage (locally):

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --workspace --summary-only
```

---

## Pull request checklist

- [ ] One concern per PR — keep it focused
- [ ] `cargo fmt` applied
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] `cargo audit` clean (no new unpatched vulnerabilities introduced)
- [ ] New code has corresponding tests (unit, integration, or both as appropriate)
- [ ] Coverage does not drop by more than 2%
- [ ] New endpoints documented in `docs/api-reference.md`
- [ ] New environment variables documented in `docs/configuration.md`
- [ ] Migrations implement both `up` and `down`, tested in both directions
- [ ] No secrets or user data in `tracing::*` calls
- [ ] No `unwrap()` outside of tests

Small PRs get reviewed and merged faster. If your change is large, split it or open a draft early to gather feedback.

---

## Code conventions

- No `unwrap()` in non-test code — use `?` or explicit error handling via `AppError` in `error.rs`
- No dead code — delete unused functions/imports, don't comment them out
- Comments explain *why*, not *what*. If you find yourself describing what the code does, the code probably needs to be clearer.
- Errors flow through `AppError` — don't add new error patterns; extend the existing enum or use `AppError::Other` with context
- Email is always stored encrypted (`email_enc`) and looked up via `email_hash`
- Any DB query touching tenant-scoped data must go through the `TenantContext` extractor (once landed in Stage 5)
- Never log a password, token, or secret. If you need to log "the user authenticated", log the user ID and the outcome, never the credential.

---

## Database migrations

Migrations live in `ovlt-core/migration/src/`. Every migration needs both `up` and `down`. After writing one:

1. Add it to `migration/src/lib.rs`
2. Test `up` by running the server locally
3. Test `down` manually with `sea-orm-cli migrate down`
4. Include the migration in your PR description with a one-line summary of what it does and any backfill implications

Schema changes that affect existing data require a backfill plan documented in the PR.

---

## Security contributions

Security work is the highest-priority category of contribution. Specific guidance:

- **Vulnerability reports:** private, via `me@shrpp.dev`. See [Security policy](docs/security.md).
- **Security hardening PRs:** open as a normal PR. Reference the relevant Stage 5 item from the roadmap. If the change is sensitive (e.g., affects token validation logic), open it as a draft and mention `@Shrpp` for early review.
- **Crypto changes:** open an issue first to discuss. Cryptographic primitives, key derivation, or anything touching `hefesto` requires deliberation before code.
- **Adding new auth flows or token types:** open an issue first. Auth surface area is the easiest place to introduce a high-impact bug.

Reporters of confirmed vulnerabilities are listed in the [Disclosed vulnerabilities](docs/security.md#disclosed-vulnerabilities) table with consent.

---

## Commit messages

Imperative present tense: `add X`, `fix Y`, `remove Z`. Subject line ≤72 chars. Body optional, for context on *why* the change matters.

Examples:
- `fix: timing-safe comparison in login_universal`
- `add: regression test for cross-tenant data access via /users endpoint`
- `refactor: extract TenantContext into its own module`

Reference issues with `Fixes #123` or `Refs #123` in the body when applicable.

---

## Questions

Open a [Discussion](https://github.com/shrpp/ovlt/discussions) — no question is too small. If you spent more than 15 minutes stuck during setup, that's a docs bug worth reporting too.

For real-time discussion or pair-debugging, mention `@Shrpp` in a Discussion or Issue.

---

## How OVLT is built

AI tools (Claude, GitHub Copilot) accelerate boilerplate, documentation, and iteration cycles. Every commit is reviewed by the author. Architecture and security decisions are the author's responsibility — as are the gaps in tests, threat model, and external review that we are working to close. Contributions are reviewed against the same conventions regardless of how they were authored: clarity, correctness, and tests.

---

Thanks for taking the time to contribute. OVLT exists because people who didn't have to spent time on it.
