# BitBox02 Rust and TypeScript library

This repo contains both a BitBox02 client library for Rust and for TypeScript. The latter is
produced from the Rust code using [Rust WASM](https://rustwasm.github.io/docs/book/).

## Rust

Check out [examples/singlethreaded.rs](examples/singlethreaded.rs) for an example.

To run the example:

    cargo run --example singlethreaded --features=usb,tokio/rt,tokio/macros

See Cargo.toml or the Makefile for further examples.

## TypeScript

If you also need a TypeScript library follow these steps as well.

Install wasm-pack using:

    cargo install wasm-pack

If not yet installed, install clang so libsecp256k1 can be cross compiled, e.g. on Ubuntu:

    sudo apt-get install clang

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

The [sandbox](sandbox/) subfolder contains a React project showcasing the TypeScript API. It
has the library in `./pkg` as a dependency.

The main entry point of the sandbox is at [./sandbox/src/App.tsx](./sandbox/src/App.tsx).

The full package API is described by the TypeScript definitions file `./pkg/bitbox_api.d.ts`.

Run the sandbox using:

    make run-sandbox

Hot-reloading is supported - you can recompile the WASM or change the sandbox files without
restarting the server.

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

This will generate/update `./src/shiftcrypto.bitbox02.rs`.
