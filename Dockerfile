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

# ─── Stage 2: OpenSSL libs ───────────────────────────────────────────────────
# Separate stage so apt guarantees the correct version regardless of what the
# builder installs. dpkg-architecture resolves the multiarch triplet at build
# time (x86_64-linux-gnu on amd64, aarch64-linux-gnu on arm64, etc.), so the
# same Dockerfile works for multi-arch builds without hardcoded paths.
FROM debian:bookworm-slim AS openssl-libs
RUN apt-get update \
 && apt-get install -y --no-install-recommends libssl3 dpkg-dev \
 && rm -rf /var/lib/apt/lists/* \
 && TRIPLET=$(dpkg-architecture -q DEB_HOST_MULTIARCH) \
 && cp /usr/lib/$TRIPLET/libssl.so.3    /libssl.so.3 \
 && cp /usr/lib/$TRIPLET/libcrypto.so.3 /libcrypto.so.3

# ─── Stage 3: Runtime ────────────────────────────────────────────────────────
# Distroless: no shell, no package manager, no setuid binaries, UID 65532.
# cc-debian12 provides glibc + libgcc; OpenSSL is copied from the openssl-libs
# stage into /usr/lib/ which is in the default ld.so search path on all arches.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=openssl-libs /libssl.so.3    /usr/lib/
COPY --from=openssl-libs /libcrypto.so.3 /usr/lib/
COPY --from=builder /app/target/release/ovlt-core /ovlt-core

EXPOSE 3000

ENTRYPOINT ["/ovlt-core"]
