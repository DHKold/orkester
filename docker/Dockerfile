# ── Stage 1: dev ──────────────────────────────────────────────────────────────
# Use this stage to compile, test and run locally on Linux via Podman/Docker.
# Mount the source tree so changes are picked up without rebuilding the image.
#
# Build context is always the project root:
#   podman build --target dev -t orkester-dev -f docker/Dockerfile .
#
# For day-to-day development, use the helper script:
#   ./docker/dev.sh
#
FROM rust:1 AS dev
WORKDIR /orkester

# Install Docker CE CLI (client only) so the container executor can talk to
# the host Podman socket mounted at /var/run/docker.sock.
# Uses Docker's official apt repo — the Debian 'docker.io' package is unreliable.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gnupg \
 && install -m 0755 -d /etc/apt/keyrings \
 && curl -fsSL https://download.docker.com/linux/debian/gpg \
      -o /etc/apt/keyrings/docker.asc \
 && chmod a+r /etc/apt/keyrings/docker.asc \
 && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] \
      https://download.docker.com/linux/debian \
      $(. /etc/os-release && echo \"$VERSION_CODENAME\") stable" \
      > /etc/apt/sources.list.d/docker.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends docker-ce-cli \
 && rm -rf /var/lib/apt/lists/*

# ── Stage 2: builder ──────────────────────────────────────────────────────────
# Compiles the release binary inside the container.
#
#   podman build -t orkester -f docker/Dockerfile .
#
FROM dev AS builder

# Cache dependencies first.
COPY Cargo.toml Cargo.lock* ./
COPY crates/common/Cargo.toml        crates/common/Cargo.toml
COPY crates/core/Cargo.toml          crates/core/Cargo.toml
COPY crates/plugin-core/Cargo.toml   crates/plugin-core/Cargo.toml
COPY crates/plugin-opa/Cargo.toml    crates/plugin-opa/Cargo.toml

RUN mkdir -p crates/common/src      && echo "pub fn _stub() {}" > crates/common/src/lib.rs   \
 && mkdir -p crates/core/src        && echo "fn main() {}"      > crates/core/src/main.rs    \
 && mkdir -p crates/plugin-core/src && echo "pub fn _stub() {}" > crates/plugin-core/src/lib.rs \
 && mkdir -p crates/plugin-opa/src  && echo "pub fn _stub() {}" > crates/plugin-opa/src/lib.rs

RUN cargo fetch

# Build the real binary.
COPY crates/ crates/
RUN cargo build --release -p orkester

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
# Minimal image containing only the compiled binary.
#
#   podman run --rm -p 8080:8080 -v ./plugins:/orkester/plugins:ro,z orkester
#
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /orkester
RUN mkdir -p plugins

COPY --from=builder /orkester/target/release/orkester /usr/local/bin/orkester

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/orkester"]
