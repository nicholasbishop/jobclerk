#!/bin/sh

set -eux

cargo fmt
cargo test
cargo clippy
cargo build
target/debug/jobclerk-server
