# ─── Stage 1: Builder ────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update \
 && apt-get install -y pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests first to cache the dependency compilation layer.
COPY Cargo.toml Cargo.lock ./
COPY ovtl-core/Cargo.toml ovtl-core/
COPY ovtl-core/migration/Cargo.toml ovtl-core/migration/

# Dummy source files — just enough for cargo to resolve and compile all deps.
RUN mkdir -p ovtl-core/src ovtl-core/migration/src \
 && printf 'fn main() {}\n' > ovtl-core/src/main.rs \
 && printf '// placeholder\n' > ovtl-core/src/lib.rs \
 && printf 'fn main() {}\n' > ovtl-core/migration/src/main.rs \
 && printf 'pub use sea_orm_migration::prelude::*;\npub struct Migrator;\n' \
      > ovtl-core/migration/src/lib.rs

# Compile dependencies only (this layer is cached unless Cargo.toml/lock changes).
RUN cargo build --release --bin ovtl-core

# Replace dummy source with real source.
COPY ovtl-core/src ovtl-core/src
COPY ovtl-core/migration/src ovtl-core/migration/src

# Touch source files so cargo detects the change and recompiles app code.
RUN touch ovtl-core/src/main.rs ovtl-core/src/lib.rs \
          ovtl-core/migration/src/main.rs ovtl-core/migration/src/lib.rs \
 && cargo build --release --bin ovtl-core

# ─── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y ca-certificates curl libssl3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -r -s /bin/false ovtl

WORKDIR /app
COPY --from=builder /app/target/release/ovtl-core ./ovtl-core

USER ovtl
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s \
  CMD curl -fsS http://localhost:3000/health || exit 1

ENTRYPOINT ["./ovtl-core"]
