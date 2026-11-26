#!/bin/bash
# SPDX-License-Identifier: Apache-2.0

set -e

features=(
  "simulator,tokio"
  "usb"
  "multithreaded,usb"
)

examples=(
  "--example singlethreaded --features=usb,tokio/rt,tokio/macros"
  "--example multithreaded --features=usb,tokio/rt,tokio/macros,tokio/rt-multi-thread,multithreaded"
  "--example btc_signtx --features=usb,tokio/rt,tokio/macros"
  "--example btc_sign_psbt --features=usb,tokio/rt,tokio/macros"
  "--example btc_sign_msg --features=usb,tokio/rt,tokio/macros"
  "--example btc_miniscript --features=usb,tokio/rt,tokio/macros"
  "--example eth --features=usb,tokio/rt,tokio/macros,rlp"
  "--example cardano --features=usb,tokio/rt,tokio/macros"
)

cargo fmt --check

cargo test --locked --lib --no-default-features
cargo check --locked --lib --no-default-features --target wasm32-unknown-unknown
for target in "$(rustc -vV | awk '$1 == "host:" { print $2 }')" wasm32-unknown-unknown; do
  dependencies=$(cargo tree --locked --no-default-features --target "$target" \
    -e normal,build --prefix none --format '{p}')
  if grep -E '^(async-trait|chrono|futures-util|getrandom|hidapi|js-sys|noise-rust-crypto|serde|serde_json|tokio|wasm-bindgen|web-sys) ' <<< "$dependencies"; then
    echo "Unexpected transport, platform, or client dependency for $target"
    exit 1
  fi
done

for feature in runtime read-write client persistence clock; do
  cargo check --locked --lib --no-default-features --features "$feature"
done

for feature_set in "${features[@]}"; do
  echo $feature_set
  cargo test --tests --locked --features="$feature_set" -- --nocapture --test-threads 1
  cargo clippy --tests --locked --features="$feature_set" -- -D warnings -A clippy::empty-docs
done

for example in "${examples[@]}"; do
    cargo test $example
    cargo clippy $example -- -D warnings -A clippy::empty-docs
done
