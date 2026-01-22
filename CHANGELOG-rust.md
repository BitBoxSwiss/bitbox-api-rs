# Changelog

## [Unreleased]
- eth: add support for streaming transactions with large data

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
