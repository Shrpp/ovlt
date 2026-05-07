# ─── Stage 1: Builder ────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

RUN apt-get update \
 && apt-get install -y pkg-config libssl-dev curl \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests first to cache the dependency compilation layer.
COPY Cargo.toml Cargo.lock ./
COPY ovlt-core/Cargo.toml ovlt-core/
COPY ovlt-core/migration/Cargo.toml ovlt-core/migration/
COPY ovlt-cli/Cargo.toml ovlt-cli/

# Dummy source files — just enough for cargo to resolve and compile all deps.
RUN mkdir -p ovlt-core/src ovlt-core/migration/src ovlt-cli/src \
 && printf 'fn main() {}\n' > ovlt-core/src/main.rs \
 && printf '// placeholder\n' > ovlt-core/src/lib.rs \
 && printf 'fn main() {}\n' > ovlt-core/migration/src/main.rs \
 && printf 'pub use sea_orm_migration::prelude::*;\npub struct Migrator;\n' \
      > ovlt-core/migration/src/lib.rs \
 && printf 'fn main() {}\n' > ovlt-cli/src/main.rs

# Compile dependencies only (this layer is cached unless Cargo.toml/lock changes).
RUN cargo build --release --bin ovlt-core

# Replace dummy source with real source.
COPY ovlt-core/src ovlt-core/src
COPY ovlt-core/migration/src ovlt-core/migration/src

# Touch source files so cargo detects the change and recompiles app code.
RUN touch ovlt-core/src/main.rs ovlt-core/src/lib.rs \
          ovlt-core/migration/src/main.rs ovlt-core/migration/src/lib.rs \
 && cargo build --release --bin ovlt-core

# ─── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y ca-certificates curl libssl3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -r -s /bin/false ovlt

WORKDIR /app
COPY --from=builder /app/target/release/ovlt-core ./ovlt-core

USER ovlt
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s \
  CMD curl -fsS http://localhost:3000/health || exit 1

ENTRYPOINT ["./ovlt-core"]
