# BitBox02 Rust and TypeScript library

This repo contains both a BitBox02 client library for Rust and for TypeScript. The latter is
produced from the Rust code using [Rust WASM](https://rustwasm.github.io/docs/book/).

## Rust

See [README-rust.md](../README-rust.md).

## TypeScript

The NPM package's README is located at [README-npm.md](../README-npm.md).

To build the TypeScript library, follow these steps:

Install wasm-pack using:

    cargo install wasm-pack

If not yet installed, install clang so libsecp256k1 can be cross compiled, e.g. on Ubuntu:

    sudo apt-get install clang

Also install the `jq` tool:

    sudo apt-get install jq

The Rust library can be compiled to WASM package including TypeScript definitions using:

    make wasm

The output of this compilation will be in `./pkg`, which is a NPM package ready to be used.

### M1 Macs

The default system clang installation currently cannot build wasm32 targets on M1 Macs.
Therefore a new clang compiler and archiver needs to be installed via:

    brew install llvm

In order to use that new clang compiler and archiver specify it when runing `make wasm`:

    AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang make wasm

## Sandbox

A demo React project showcasing the TypeScript API. See [sandbox](../sandbox).

## Command to update the BitBox02 protobuf message files

Normally, Prost protobuf files are generated in `build.rs` during each compilation. This has a
number of downsides:

- The generated .rs file is not committed and depends on the particular version of `prost-build`
  that is used, as well as on the system installation of the `protoc` compiler.
- As a consequence, re-building older version of this library might become tricky if the particular
  versions of these tools are not easy to install in the future.
- Downstream projects need to install `protoc` in order to build this library, on dev-machines, in
  CI scripts, etc.

By pre-generating the file and making it a regular committed source file, these problems fall away.

As a maintainer/developer of this library, to update the protobuf messages, follow these steps:

Clone the [BitBox02 firmware repo](https://github.com/digitalbitbox/bitbox02-firmware):

Make sure you have `protoc` installed:

On Ubuntu:

    sudo apt-get install protobuf-compiler

On MacOS:

    brew install protobuf

Install `rust-script`:

    cargo install rust-script

Then:

```sh
rm -rf messages/*.proto
cp /path/to/bitbox02-firmware/messages/*.proto messages/
rm messages/backup.proto
make build-protos
```

This will generate/update [src/shiftcrypto.bitbox02.rs](../src/shiftcrypto.bitbox02.rs).
