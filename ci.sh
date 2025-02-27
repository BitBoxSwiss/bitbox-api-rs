#!/bin/bash

set -e

features=(
  "simulator,tokio"
  "usb"
  "wasm"
  "multithreaded,usb"
)

examples=(
  "--example singlethreaded --features=usb,tokio/rt,tokio/macros"
  "--example multithreaded --features=usb,tokio/rt,tokio/macros,tokio/rt-multi-thread,multithreaded"
  " --example btc_signtx --features=usb,tokio/rt,tokio/macros"
  "--example btc_sign_psbt --features=usb,tokio/rt,tokio/macros"
  "--example btc_sign_msg --features=usb,tokio/rt,tokio/macros"
  "--example btc_miniscript --features=usb,tokio/rt,tokio/macros"
  "--example eth --features=usb,tokio/rt,tokio/macros,rlp"
  "--example cardano --features=usb,tokio/rt,tokio/macros"
)

cargo fmt --check

for feature_set in "${features[@]}"; do
  echo $feature_set
  cargo test --tests --locked --features="$feature_set" -- --nocapture --test-threads 1
  cargo clippy --tests --locked --features="$feature_set" -- -D warnings -A clippy::empty-docs
done

for example in "${examples[@]}"; do
    cargo test $example
    cargo clippy $example -- -D warnings -A clippy::empty-docs
done
