#!/bin/sh

set -eux

cargo fmt
cargo test
cargo clippy
cargo run --example server
