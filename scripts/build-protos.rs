//! ```cargo
//! [dependencies]
//! # If you change this, also change the version of prost in Cargo.toml.
//! prost-build = { version = "0.14" }
//! ```

// SPDX-License-Identifier: Apache-2.0

use std::io::Result;

fn generate_proto() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.out_dir("src/");

    config.compile_protos(&["messages/hww.proto"], &["messages/"])
}

fn main() -> Result<()> {
    generate_proto()
}
