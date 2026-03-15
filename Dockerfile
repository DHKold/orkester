# Production image for Orkester
#
# glibc binary + distroless runtime so the dynamic linker is present and
# .so plugins can be loaded at runtime via libloading.
#
#   podman build -t orkester:latest .
#
#   podman run --rm -p 8080:8080 \
#     -v ./config.yaml:/orkester/config.yaml:ro,z \
#     -v ./plugins:/orkester/plugins:ro,z \
#     -v orkester-data:/orkester/data:z \
#     orkester:latest -c /orkester/config.yaml

# ── Stage 1: build ────────────────────────────────────────────────────────────
# Pin to Bookworm so the glibc version compiled against matches the runtime.
# rust:1-slim defaults to the latest Debian which may ship a newer glibc than
# distroless/cc-debian12 (Bookworm, glibc 2.36).
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

COPY . .
RUN cargo build --release -p orkester \
 && cargo build --release -p orkester-plugin-core \
 && cargo build --release -p orkester-plugin-k8s

# Prepare runtime filesystem layout and credentials while we have a shell.
RUN echo 'orkester:x:10001:10001:Orkester:/orkester:/sbin/nologin' > /etc/orkester-passwd \
 && echo 'orkester:x:10001:' > /etc/orkester-group \
 && mkdir -p /build/rootfs/orkester/plugins /build/rootfs/orkester/data \
 && cp target/release/liborkester_plugin_core.so /build/rootfs/orkester/plugins/ \
 && cp target/release/liborkester_plugin_k8s.so  /build/rootfs/orkester/plugins/ \
 && chmod 755 /build/rootfs/orkester/plugins /build/rootfs/orkester/data \
 && chmod 644 /build/rootfs/orkester/plugins/*.so \
 && chmod 755 target/release/orkester

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
# gcr.io/distroless/cc-debian12 provides:
#   • glibc  — required by glibc-linked Rust binaries and .so plugins
#   • libgcc_s — unwinding support
#   • ca-certificates — TLS to k8s API / external services
#   • the ELF dynamic linker (/lib64/ld-linux-x86-64.so.2)
# :nonroot sets USER 65532 by default; we override with our named user below.
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /etc/orkester-passwd                /etc/passwd
COPY --from=builder /etc/orkester-group                 /etc/group
COPY --chown=10001:10001 --from=builder /build/rootfs/orkester /orkester
COPY --from=builder /build/target/release/orkester      /usr/local/bin/orkester

VOLUME ["/orkester/plugins", "/orkester/data"]
WORKDIR /orkester
EXPOSE 8080
USER orkester
ENTRYPOINT ["/usr/local/bin/orkester"]
