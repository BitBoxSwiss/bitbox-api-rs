# BitBox02 Rust library

A Rust library to interact with the BitBox02 hardware wallet.

The TypeScript library is maintained in
[BitBoxSwiss/bitbox-api-ts](https://github.com/BitBoxSwiss/bitbox-api-ts/).

Check out [examples/singlethreaded.rs](examples/singlethreaded.rs) for an example.

To run the example:

    cargo run --example singlethreaded --features=usb,tokio/rt,tokio/macros

See [Cargo.toml](Cargo.toml) for further examples.

## Simulator tests

The integration tests in [tests/](tests/) run against BitBox02 simulators. The simulators are
automatically downloaded based on [tests/simulators.json](tests/simulators.json), and the tests
run against each one.

To run them, use:

    cargo test --features=simulator,tokio -- --test-threads 1

Use `--nocapture` to also see some useful simulator output.

    cargo test --features=simulator,tokio -- --test-threads 1 --nocapture

If you want to test against a custom simulator build (e.g. when developing new firmware features),
you can run:

    SIMULATOR=/path/to/simulator cargo test --features=simulator,tokio -- --test-threads 1

In this case, only the given simulator will be used, and the ones defined in simulators.json will be
ignored.

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

This will generate/update [src/shiftcrypto.bitbox02.rs](src/shiftcrypto.bitbox02.rs).
