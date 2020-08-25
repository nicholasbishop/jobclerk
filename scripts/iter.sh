#!/bin/sh

set -eux

cargo fmt
# Build before test so that the integration test works
cargo build
cargo test
cargo clippy
target/debug/jobclerk-server
