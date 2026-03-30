#!/usr/bin/env bash
# dev/build-and-run.sh — Build both Rust workspaces, copy artefacts to /orkester/bin,
# then launch orkester with the workaholic config.
#
# Run from inside the orkester-dev container:
#   bash /orkester/dev/build-and-run.sh           # debug build
#   bash /orkester/dev/build-and-run.sh --release  # release build (slower to compile)
#
# The script must be run as a user that can write to /orkester/bin.

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────

ORKESTER_ROOT="/orkester"
BIN_DIR="${ORKESTER_ROOT}/bin"
RELEASE=0

# Parse flags
for arg in "$@"; do
    case "$arg" in
        --release) RELEASE=1 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

CARGO_PROFILE="debug"
CARGO_FLAGS=""
if [[ "${RELEASE}" -eq 1 ]]; then
    CARGO_PROFILE="release"
    CARGO_FLAGS="--release"
fi

ORKESTER_WS="${ORKESTER_ROOT}/rust/orkester"
WORKAHOLIC_WS="${ORKESTER_ROOT}/rust/workaholic"

echo ">>> Build profile: ${CARGO_PROFILE}"
echo ">>> Bin dir:       ${BIN_DIR}"

# ── 1. Build orkester host + sample plugin ────────────────────────────────────

echo ""
echo "=== Building orkester (host + sample plugin) ==="
(
    cd "${ORKESTER_WS}"
    cargo build ${CARGO_FLAGS} \
        -p orkester-host \
        -p orkester-plugin-sample \
        -p orkester-plugin-metrics 2>&1
)

# ── 2. Build workaholic plugin ────────────────────────────────────────────────

echo ""
echo "=== Building workaholic plugin ==="
(
    cd "${WORKAHOLIC_WS}"
    cargo build ${CARGO_FLAGS} -p orkester-plugin-workaholic 2>&1
)

# ── 3. Copy artefacts to bin dir ──────────────────────────────────────────────

echo ""
echo "=== Copying artefacts to ${BIN_DIR} ==="

mkdir -p "${BIN_DIR}"

# Orkester host binary
cp -v "${ORKESTER_WS}/target/${CARGO_PROFILE}/orkester" \
      "${BIN_DIR}/orkester"

# Sample plugin (provides LoggingServer, PingServer, RestServer)
cp -v "${ORKESTER_WS}/target/${CARGO_PROFILE}/liborkester_plugin_sample.so" \
      "${BIN_DIR}/"

# Metrics plugin
cp -v "${ORKESTER_WS}/target/${CARGO_PROFILE}/liborkester_plugin_metrics.so" \
      "${BIN_DIR}/"

# Workaholic plugin
cp -v "${WORKAHOLIC_WS}/target/${CARGO_PROFILE}/liborkester_plugin_workaholic.so" \
      "${BIN_DIR}/"

echo ""
echo "=== Bin dir contents ==="
ls -lh "${BIN_DIR}/"*.so "${BIN_DIR}/orkester" 2>/dev/null || true

# ── 4. Run orkester ───────────────────────────────────────────────────────────

CONFIG="${BIN_DIR}/workaholic.yaml"

if [[ ! -f "${CONFIG}" ]]; then
    echo "ERROR: config not found at ${CONFIG}" >&2
    exit 1
fi

echo ""
echo "=== Starting orkester (config: ${CONFIG}) ==="
exec "${BIN_DIR}/orkester" --config "${CONFIG}"
