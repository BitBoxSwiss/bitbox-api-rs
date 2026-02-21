# Changelog

## [Unreleased]
- eth: add support for streaming transactions with large data
- eth: add optional `useAntiklepto` argument to `ethSignTypedMessage()` (set to `false` for
  deterministic typed-message signatures, firmware >=9.26.0)

## 0.12.0
- btc: add support for OP_RETURN outputs
- add `changePassword()` to change the device password (firmware >=9.25.0)

## 0.11.0
- btc: add support for OP_RETURN outputs

## 0.10.1
- package.json: use "main" instead of "module" to fix compatiblity with vitest

## 0.10.0
- Add `bip85AppBip39()`
- Add support for BitBox02 Nova

## 0.9.1
- WebHID: Automatically connect to a previoulsy connected device

## 0.9.0
- cardano: add support for 258-tagged sets

## 0.8.0
- cardano: allow vote delegation

## 0.7.0
- btc: handle error when an input's previous transaction is required but missing
- btc: add support for regtest
- btc: add support for Taproot wallet policies
- eth: add method to help clients identify and specify address case (upper/lower/mixed)

## 0.6.0

- btc: add support for multisig script configs
