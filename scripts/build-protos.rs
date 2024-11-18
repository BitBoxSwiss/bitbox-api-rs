//! ```cargo
//! [dependencies]
//! # If you change this, also change the version of prost in Cargo.toml.
//! prost-build = { version = "0.13" }
//! ```

use std::io::Result;

fn add_serde_attrs(c: &mut prost_build::Config) {
    let type_attrs = &[
        (".", "derive(serde::Serialize, serde::Deserialize)"),
        (".", "serde(rename_all = \"camelCase\")"),
        (
            "shiftcrypto.bitbox02.KeyOriginInfo",
            "serde(try_from = \"crate::btc::KeyOriginInfo\")",
        ),
        // Bitcoin
        (
            "shiftcrypto.bitbox02.BTCScriptConfig",
            "serde(try_from = \"crate::btc::SerdeScriptConfig\")",
        ),
        // Cardano
        (
            "shiftcrypto.bitbox02.CardanoScriptConfig",
            "serde(try_from = \"crate::cardano::SerdeScriptConfig\")",
        ),
        (
            "shiftcrypto.bitbox02.CardanoSignTransactionRequest.Output",
            "serde(default)", // allow skipping scriptConfig, assetGroups
        ),
        (
            "shiftcrypto.bitbox02.CardanoSignTransactionRequest.Certificate",
            "serde(try_from = \"crate::cardano::SerdeCert\")",
        ),
    ];
    let field_attrs = &[
        (
            "keypath",
            "serde(deserialize_with = \"crate::keypath::serde_deserialize\")",
        ),
        // Bitcoin
        (
            "shiftcrypto.bitbox02.BTCScriptConfig.config.simple_type",
            "serde(deserialize_with = \"crate::btc::serde_deserialize_simple_type\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCScriptConfig.config.multisig",
            "serde(deserialize_with = \"crate::btc::serde_deserialize_multisig\")",
        ),
        (
            "shiftcrypto.bitbox02.RootFingerprintResponse.fingerprint",
            "serde(deserialize_with = \"hex::serde::deserialize\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCPubRequest.XPubType.CAPITAL_VPUB",
            "serde(rename = \"Vpub\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCPubRequest.XPubType.CAPITAL_ZPUB",
            "serde(rename = \"Zpub\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCPubRequest.XPubType.CAPITAL_UPUB",
            "serde(rename = \"Upub\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCPubRequest.XPubType.CAPITAL_YPUB",
            "serde(rename = \"Ypub\")",
        ),
        // Cardano
        (
            "shiftcrypto.bitbox02.CardanoNetwork.CardanoMainnet",
            "serde(rename = \"mainnet\")",
        ),
        (
            "shiftcrypto.bitbox02.CardanoNetwork.CardanoTestnet",
            "serde(rename = \"testnet\")",
        ),
        (
            "shiftcrypto.bitbox02.CardanoSignTransactionRequest.network",
            "serde(deserialize_with = \"crate::cardano::serde_deserialize_network\")",
        ),
        (
            "shiftcrypto.bitbox02.CardanoSignTransactionRequest.allow_zero_ttl",
            "serde(rename = \"allowZeroTTL\")",
        ),
        (
            "keypath_payment",
            "serde(deserialize_with = \"crate::keypath::serde_deserialize\")",
        ),
        (
            "keypath_stake",
            "serde(deserialize_with = \"crate::keypath::serde_deserialize\")",
        ),
        (
            "shiftcrypto.bitbox02.CardanoSignTransactionRequest.Certificate.VoteDelegation.type",
            "serde(deserialize_with = \"crate::cardano::serde_deserialize_drep_type\")",
        ),
    ];

    for (path, attr) in type_attrs {
        c.type_attribute(path, &format!("#[cfg_attr(feature=\"wasm\", {})]", attr));
    }

    for (path, attr) in field_attrs {
        c.field_attribute(path, &format!("#[cfg_attr(feature=\"wasm\", {})]", attr));
    }
}

fn generate_proto() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.out_dir("src/");
    add_serde_attrs(&mut config);

    config.compile_protos(&["messages/hww.proto"], &["messages/"])
}

fn main() -> Result<()> {
    generate_proto()
}
