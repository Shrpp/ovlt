# Contributing to OVLT

OVLT is a small project with big ambitions — a developer-first auth service that just works, without the Keycloak complexity or the Auth0 price tag. Every contribution, however small, moves that forward.

Licensed under the [MIT License](LICENSE).

---

## Where to start

Not sure what to work on? Here are the best entry points:

- **Good first issues** — look for the `good first issue` label on GitHub. These are scoped, self-contained, and well-documented.
- **The roadmap** — Stage 5 (Production Hardening) is actively in progress. Integration tests, password history enforcement, and expanded audit log are up for grabs.
- **Docs** — if something confused you during setup, fix it. Doc PRs are always welcome and always merged fast.
- **Bug reports** — found something broken? Open an issue. Clear reproduction steps + environment details (OS, Rust version, PostgreSQL version) make fixing it 10x faster.

If you want to work on something bigger (new auth flow, encryption change, multi-tenancy logic), **open an issue first** to align before building. Saves everyone time.

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
cargo clippy -- -D warnings
```

Format:

```bash
cargo fmt
```

---

## Pull request checklist

- [ ] One concern per PR — keep it focused
- [ ] `cargo fmt` applied
- [ ] `cargo clippy -- -D warnings` passes
- [ ] New endpoints documented in `README.md` API table
- [ ] Migrations implement both `up` and `down`, tested in both directions
- [ ] No secrets or user data in `tracing::*` calls
- [ ] No `unwrap()` outside of tests

Small PRs get reviewed and merged faster. If your change is large, consider splitting it or opening a draft early to get feedback.

---

## Code conventions

- No `unwrap()` in non-test code — use `?` or explicit error handling
- No dead code — delete unused functions/imports, don't comment them out
- No comments that explain *what* the code does — only *why*, and only when it's non-obvious
- Errors go through `AppError` in `error.rs` — don't add new error patterns
- Email is always stored encrypted (`email_enc`) and looked up via `email_hash`

---

## Database migrations

Migrations live in `ovlt-core/migration/src/`. Every migration needs both `up` and `down`. After writing one:

1. Add it to `migration/src/lib.rs`
2. Test `up` by running the server locally
3. Test `down` manually with `sea-orm-cli migrate down`

---

## Commit messages

Imperative present tense: `add X`, `fix Y`, `remove Z`. Subject line ≤72 chars. Optional body for context on *why*, not *what*.

---

## Security issues

Do **not** open a public issue for vulnerabilities. Email **me@shrpp.dev** with details — you'll get a response within 48h.

---

## Questions?

Open a [Discussion](https://github.com/shrpp/ovlt/discussions) — no question is too small. If you spent more than 15 minutes stuck on something during setup, that's a docs bug worth reporting too.
