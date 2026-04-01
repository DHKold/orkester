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
DEV_DIR="${ORKESTER_ROOT}/dev"
BIN_DIR="${ORKESTER_ROOT}/dev/bin"

CONFIG="${DEV_DIR}/workaholic.yaml"

if [[ ! -f "${CONFIG}" ]]; then
    echo "ERROR: config not found at ${CONFIG}" >&2
    exit 1
fi

echo ""
echo "=== Starting orkester (config: ${CONFIG}) ==="
exec "${BIN_DIR}/orkester" --config "${CONFIG}"