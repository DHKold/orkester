#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${SCRIPT_DIR}/target"
ORKESTER_CONFIG="${SCRIPT_DIR}/orkester-config.yaml"

cd "${SCRIPT_DIR}"

# Build the dynamic libraries for the orkester host and plugins, then copy them to target/test
cargo build --release -p orkester-plugin-core
cargo build --release -p orkester-plugin-k8s

cp -v "${TARGET_DIR}/release/*.so" "${TARGET_DIR}/test/"

# Run the orkester host with the test configuration that loads the plugins from target/test
cargo run --release -- -c "${ORKESTER_CONFIG}"