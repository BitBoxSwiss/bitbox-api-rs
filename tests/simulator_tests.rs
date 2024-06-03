#![cfg(feature = "simulator")]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

use bitcoin::hashes::Hash;
use std::process::Command;
use std::str::FromStr;

use bitbox_api::pb;

type PairedBitBox = bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>;

async fn test_btc(bitbox: &PairedBitBox) {
    // btc_xpub
    {
        let xpub = bitbox
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
    }
    // btc_address
    {
        let address = bitbox
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
    }
    // btc_sign_message
    {
        let xpub_str = bitbox
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
            .inner;

        let sign_result = bitbox
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
    }
}

#[tokio::test]
async fn test_device() {
    let _server = Command::new("./tests/simulator")
        .spawn()
        .expect("failed to start server");

    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox =
        bitbox_api::BitBox::<bitbox_api::runtime::TokioRuntime>::from_simulator(None, noise_config)
            .await
            .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();

    let device_info = paired_bitbox.device_info().await.unwrap();

    assert_eq!(device_info.name, "My BitBox");
    assert_eq!(paired_bitbox.product(), bitbox_api::Product::BitBox02Multi);

    assert!(paired_bitbox.restore_from_mnemonic().await.is_ok());

    // --- Tests that run on the initialized/seeded device follow.
    // --- The simulator is initialized with the following mnemonic:
    // --- boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide

    assert_eq!(
        paired_bitbox.root_fingerprint().await.unwrap().as_str(),
        "4c00739d"
    );

    assert!(paired_bitbox.show_mnemonic().await.is_ok());

    test_btc(&paired_bitbox).await;
}
