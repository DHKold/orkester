#! /bin/bash

export RUSTFLAGS=-Awarnings

./docker/build-plugins.sh
cargo run -- -c docker/config.yaml