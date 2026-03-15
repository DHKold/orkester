#! /bin/bash

export RUSTFLAGS=-Awarnings

./dev/build-plugins.sh
cd rust
cargo run -- -c /orkester/dev/config.yaml