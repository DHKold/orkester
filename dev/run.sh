#! /bin/bash

export RUSTFLAGS=-Awarnings

./dev/build-plugins.sh
cargo run -- -c dev/config.yaml