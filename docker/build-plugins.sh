#!/usr/bin/env bash
# docker/build-plugins.sh — Build all plugin crates and install them into ./plugins/
#
# Run from the project root inside the dev container:
#   ./docker/build-plugins.sh           # debug build (default)
#   ./docker/build-plugins.sh --release # release build

set -euo pipefail

RELEASE_FLAG=""
BUILD_DIR="debug"

for arg in "$@"; do
    if [ "$arg" = "--release" ]; then
        RELEASE_FLAG="--release"
        BUILD_DIR="release"
    fi
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$PROJECT_ROOT"

PLUGINS_DIR="${PROJECT_ROOT}/docker/plugins"
mkdir -p "$PLUGINS_DIR"

echo ">>> Building plugins (${BUILD_DIR})..."

# List all plugin crates (any workspace member whose name starts with orkester-plugin-)
PLUGIN_CRATES=(
    orkester-plugin-core
)

for crate in "${PLUGIN_CRATES[@]}"; do
    echo "    Building ${crate}..."
    cargo build ${RELEASE_FLAG} -p "$crate"
done

echo ">>> Installing .so files into ${PLUGINS_DIR}/"

# Rust converts hyphens to underscores for the lib filename.
for crate in "${PLUGIN_CRATES[@]}"; do
    lib_name="lib$(echo "$crate" | tr '-' '_').so"
    src="target/${BUILD_DIR}/${lib_name}"
    if [ -f "$src" ]; then
        cp "$src" "$PLUGINS_DIR/"
        echo "    Installed ${lib_name}"
    else
        echo "    Skipped ${lib_name} (not found — crate may be a stub)"
    fi
done

echo ">>> Done. Plugins in ${PLUGINS_DIR}:"
ls -lh "$PLUGINS_DIR"
