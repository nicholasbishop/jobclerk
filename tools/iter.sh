#!/bin/sh

set -eux

cargo fmt && cargo test && cargo clippy && RUST_LOG=actix_web=debug cargo run --bin server
