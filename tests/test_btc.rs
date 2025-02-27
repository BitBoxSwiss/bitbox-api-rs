#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod util;

use util::test_initialized_simulators;

use bitcoin::hashes::Hash;
use std::str::FromStr;

use bitbox_api::pb;

#[tokio::test]
async fn test_btc_xpub() {
    test_initialized_simulators(async |paired_bitbox| {
        let xpub = paired_bitbox
            .btc_xpub(
                pb::BtcCoin::Tbtc,
                &"m/49'/1'/0'".try_into().unwrap(),
                pb::btc_pub_request::XPubType::Ypub,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            xpub.as_str(),
            "ypub6WqXiL3fbDK5QNPe3hN4uSVkEvuE8wXoNCcecgggSuKVpU3Kc4fTvhuLgUhtnbAdaTb9gpz5PQdvzcsKPTLgW2CPkF5ZNRzQeKFT4NSc1xN",
        );
    })
    .await
}

#[tokio::test]
async fn test_btc_address() {
    test_initialized_simulators(async |paired_bitbox| {
        let address = paired_bitbox
            .btc_address(
                pb::BtcCoin::Tbtc,
                &"m/84'/1'/0'/1/10".try_into().unwrap(),
                &bitbox_api::btc::make_script_config_simple(
                    pb::btc_script_config::SimpleType::P2wpkh,
                ),
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            address.as_str(),
            "tb1qq064dxjgl9h9wzgsmzy6t6306qew42w9ka02u3"
        );
    })
    .await
}

#[tokio::test]
async fn test_btc_sign_message() {
    test_initialized_simulators(async |paired_bitbox| {
        let xpub_str = paired_bitbox
            .btc_xpub(
                pb::BtcCoin::Btc,
                &"m/49'/0'/0'".try_into().unwrap(),
                pb::btc_pub_request::XPubType::Xpub,
                false,
            )
            .await
            .unwrap();
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let pubkey = bitcoin::bip32::Xpub::from_str(&xpub_str)
            .unwrap()
            .derive_pub(
                &secp,
                &"m/0/10".parse::<bitcoin::bip32::DerivationPath>().unwrap(),
            )
            .unwrap()
            .to_pub()
            .0;

        let sign_result = paired_bitbox
            .btc_sign_message(
                pb::BtcCoin::Btc,
                pb::BtcScriptConfigWithKeypath {
                    script_config: Some(bitbox_api::btc::make_script_config_simple(
                        pb::btc_script_config::SimpleType::P2wpkhP2sh,
                    )),
                    keypath: bitbox_api::Keypath::try_from("m/49'/0'/0'/0/10")
                        .unwrap()
                        .to_vec(),
                },
                b"message",
            )
            .await
            .unwrap();

        pubkey
            .verify(
                &secp,
                &bitcoin::secp256k1::Message::from_digest(
                    bitcoin::hashes::sha256d::Hash::hash(
                        b"\x18Bitcoin Signed Message:\n\x07message",
                    )
                    .to_byte_array(),
                ),
                &bitcoin::secp256k1::ecdsa::Signature::from_compact(&sign_result.sig).unwrap(),
            )
            .unwrap();
    })
    .await
}
