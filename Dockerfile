# Production image for Orkester
#
# glibc binary + distroless runtime so the dynamic linker is present and
# .so plugins can be loaded at runtime via libloading.
#
#   podman build -t orkester:latest .
#
#   podman run --rm -p 8080:8080 \
#     -v ./config.yaml:/orkester/config.yaml:ro,z \
#     -v orkester-data:/orkester/data:z \
#     orkester:latest -c /orkester/config.yaml

# ── Stage 1: build ────────────────────────────────────────────────────────────
# Pin to Bookworm so the glibc version compiled against matches the runtime.
# rust:1-slim defaults to the latest Debian which may ship a newer glibc than
# distroless/cc-debian12 (Bookworm, glibc 2.36).
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

# Only copy Rust sources — changes to helm charts, ui, etc. don't
# invalidate the build layer.
COPY rust/ .

# ── orkester workspace: host binary + sample + metrics plugins ─────────────────
# Cache mounts survive across builds; binaries are extracted via cp before the
# ephemeral target/ mount is discarded.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/orkester/target \
    cd /build/orkester \
 && cargo build --release \
        -p orkester-host \
        -p orkester-plugin-sample \
        -p orkester-plugin-metrics \
 && cp target/release/orkester                        /usr/local/bin/orkester \
 && cp target/release/liborkester_plugin_sample.so    /tmp/ \
 && cp target/release/liborkester_plugin_metrics.so   /tmp/

# ── workaholic workspace: workaholic plugin ────────────────────────────────────
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/workaholic/target \
    cd /build/workaholic \
 && cargo build --release -p orkester-plugin-workaholic \
 && cp target/release/liborkester_plugin_workaholic.so /tmp/

# Prepare runtime filesystem layout and credentials while we have a shell.
RUN echo 'orkester:x:10001:10001:Orkester:/orkester:/sbin/nologin' > /etc/orkester-passwd \
 && echo 'orkester:x:10001:' > /etc/orkester-group \
 && mkdir -p /build/rootfs/orkester/plugins /build/rootfs/orkester/data \
 && cp /tmp/liborkester_plugin_sample.so     /build/rootfs/orkester/plugins/ \
 && cp /tmp/liborkester_plugin_metrics.so    /build/rootfs/orkester/plugins/ \
 && cp /tmp/liborkester_plugin_workaholic.so /build/rootfs/orkester/plugins/ \
 && chmod 755 /build/rootfs/orkester/plugins /build/rootfs/orkester/data \
 && chmod 644 /build/rootfs/orkester/plugins/*.so

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
# gcr.io/distroless/cc-debian12 provides:
#   • glibc  — required by glibc-linked Rust binaries and .so plugins
#   • libgcc_s — unwinding support
#   • ca-certificates — TLS to k8s API / external services
#   • the ELF dynamic linker (/lib64/ld-linux-x86-64.so.2)
# :nonroot sets USER 65532 by default; we override with our named user below.
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /etc/orkester-passwd                      /etc/passwd
COPY --from=builder /etc/orkester-group                       /etc/group
COPY --chown=10001:10001 --from=builder /build/rootfs/orkester /orkester
COPY --from=builder /usr/local/bin/orkester                   /usr/local/bin/orkester
COPY --chown=10001:10001 ui/                                  /orkester/ui/

VOLUME ["/orkester/data"]
WORKDIR /orkester
EXPOSE 8080
USER orkester
ENTRYPOINT ["/usr/local/bin/orkester"]
