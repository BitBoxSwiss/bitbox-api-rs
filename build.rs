use std::io::Result;

#[cfg(feature = "serde")]
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
    ];
    let field_attrs = &[
        (
            "keypath",
            "serde(deserialize_with = \"crate::keypath::serde_deserialize\")",
        ),
        (
            "shiftcrypto.bitbox02.BTCScriptConfig.config.simple_type",
            "serde(deserialize_with = \"crate::btc::serde_deserialize_simple_type\")",
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
    ];

    for (path, attr) in type_attrs {
        c.type_attribute(path, &format!("#[{}]", attr));
    }

    for (path, attr) in field_attrs {
        c.field_attribute(path, &format!("#[{}]", attr));
    }
}

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    #[cfg(feature = "serde")]
    add_serde_attrs(&mut config);

    config.compile_protos(&["src/messages/hww.proto"], &["src/messages/"])?;
    Ok(())
}
