#!/bin/bash

set -e

features=(
  "usb"
  "usb,serde"
  "wasm"
  "multithreaded,usb,serde"
)

examples=(
  "--example singlethreaded --features=usb,tokio/rt,tokio/macros"
  "--example multithreaded --features=usb,tokio/rt,tokio/macros,tokio/rt-multi-thread,multithreaded"
  " --example btc_signtx --features=usb,tokio/rt,tokio/macros"
  "--example btc_sign_psbt --features=usb,tokio/rt,tokio/macros"
  "--example btc_miniscript --features=usb,tokio/rt,tokio/macros"
  "--example eth --features=usb,tokio/rt,tokio/macros,rlp"
  "--example cardano --features=usb,tokio/rt,tokio/macros"
)

cargo fmt --check

for feature_set in "${features[@]}"; do
  echo $feature_set
  cargo test --locked --features="$feature_set" --all-targets
  cargo clippy --locked --features="$feature_set" --all-targets -- -D warnings
done

for example in "${examples[@]}"; do
    cargo test $example
    cargo clippy $example -- -D warnings
done
