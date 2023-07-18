#!/bin/bash

set -e

cargo test --locked --all-features --examples --all-targets
cargo clippy --locked --all-features --examples --all-targets -- -D warnings
cargo fmt --check
