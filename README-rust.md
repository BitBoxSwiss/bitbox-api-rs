# BitBox02 library

A library to interact with the BitBox02 hardware wallet.

Check out [examples/singlethreaded.rs](examples/singlethreaded.rs) for an example.

To run the example:

    cargo run --example singlethreaded --features=usb,tokio/rt,tokio/macros

See [Cargo.toml](Cargo.toml) for further examples.

## Simulator tests

tests/simulator_tests.rs runs a set of integration tests against BitBox02 simulators. They are
automatically downloaded based on [tests/simulators.json](tests/simulators.json), and each one is
tested with.

To run them, use:

    cargo test --features=simulator,tokio -- --test-threads 1

Use `--nocapture` to also see some useful simulator output.

    cargo test --features=simulator,tokio -- --test-threads 1 --nocapture

If you want to test against a custom simulator build (e.g. when developing new firmware features),
you can run:

    SIMULATOR=/path/to/simulator cargo test --features=simulator,tokio

In this case, only the given simulator will be used, and the ones defined in simulators.json will be
ignored.
