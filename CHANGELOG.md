# Changelog

## [Unreleased]
- Remove the `wasm` feature, the public `wasm` module, and the TypeScript/WASM package. The
  TypeScript library is now maintained in
  [BitBoxSwiss/bitbox-api-ts](https://github.com/BitBoxSwiss/bitbox-api-ts/).
- Remove the `serde::Serialize` implementation for `btc::SignMessageSignature`, including its
  camelCase field naming. This affects native Rust users as well.
- Remove the WASM-only Serde implementations: `Serialize` and `Deserialize` for protobuf types,
  and `Deserialize` for `Keypath`, `btc::KeyOriginInfo`, `eth::Transaction`, and
  `eth::EIP1559Transaction`.
- Remove the WASM-only `js_code()` methods on `error::Error`, `error::BitBoxError`, and
  `btc::PsbtError`.
- Stop enabling Bitcoin's `base64` feature for library consumers. Applications using base64 PSBT
  parsing or formatting must enable `bitcoin/base64` in their own dependencies. The PSBT example
  continues to enable it through development dependencies.
- Stop enabling the dependency features previously supplied by `wasm`: `bitcoin/serde`,
  `hex/serde`, `getrandom/js`, and `bitcoin/secp-lowmemory`. Consumers relying on these features
  must enable them directly on their own dependencies.
- Build only a Rust library by default; Cargo no longer also generates a `cdylib` artifact.
- Use Cargo's default release profile instead of the WASM-oriented size optimization settings.

## 0.13.1
- Restrict persisted Noise config files to private permissions on Unix.
- Validate ECDSA signatures and recovery IDs in Anti-Klepto and direct signing flows.

## 0.13.0
- Add `BitBox::from_transport()`
- Serialize public API calls that talk to the device to avoid interleaving request/response
  sequences and breaking the device communication state.

## 0.12.0
- eth: add support for streaming transactions and EIP-712 typed data with large data
- eth: add `use_antiklepto` toggle to `eth_sign_typed_message()` (set `false` for deterministic
  typed-message signatures, firmware >=9.26.0)

## 0.11.0
- btc: add support for OP_RETURN outputs
- add `change_password()` to change the device password (firmware >=9.25.0)

## 0.10.0
- Add `btc_xpubs()`

## 0.9.0
- Add support for BitBox02 Nova

## 0.8.0
- Add `bip85_app_bip39()`
- Make the `simulator` feature work with the `multithreaded` feature

## 0.7.0
- cardano: add support for 258-tagged sets

## 0.6.0
- btc: handle error when an input's previous transaction is required but missing
- btc: add support for regtest
- btc: add support for Taproot wallet policies
- cardano: added support for vote delegation
- eth: add method to help clients identify and specify address case (upper/lower/mixed)

## 0.5.0

- btc: add `make_script_config_multisig()`
