use std::str::FromStr;

use bitbox_api::pb;

async fn get_bitbox02() -> bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime> {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox = bitbox_api::BitBox::<bitbox_api::runtime::TokioRuntime>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02().unwrap(),
        noise_config,
    )
    .await
    .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    pairing_bitbox.wait_confirm().await.unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let paired_bitbox = get_bitbox02().await;

    let coin = pb::BtcCoin::Tbtc;
    let policy = "wsh(andor(pk(@0/**),older(12960),pk(@1/**)))";

    let our_root_fingerprint = paired_bitbox.root_fingerprint().await.unwrap();

    let keypath_account: bitbox_api::Keypath = "m/48'/1'/0'/3'".try_into().unwrap();
    let our_xpub_str = paired_bitbox
        .btc_xpub(
            coin,
            &keypath_account,
            pb::btc_pub_request::XPubType::Tpub,
            false,
        )
        .await
        .unwrap();
    let our_xpub = bitbox_api::btc::ExtendedPubKey::from_str(&our_xpub_str).unwrap();
    let some_xpub = bitbox_api::btc::ExtendedPubKey::from_str("tpubDFgycCkexSxkdZfeyaasDHityE97kiYM1BeCNoivDHvydGugKtoNobt4vEX6YSHNPy2cqmWQHKjKxciJuocepsGPGxcDZVmiMBnxgA1JKQk").unwrap();

    let keys = &[
        // Our key: root fingerprint and keypath are required.
        bitbox_api::btc::KeyOriginInfo {
            root_fingerprint: Some(
                bitbox_api::btc::Fingerprint::from_str(&our_root_fingerprint).unwrap(),
            ),
            keypath: Some(keypath_account.clone()),
            xpub: our_xpub,
        },
        // Foreign key: root fingerprint and keypath are optional.
        bitbox_api::btc::KeyOriginInfo {
            root_fingerprint: None,
            keypath: None,
            xpub: some_xpub,
        },
    ];
    let policy_config = bitbox_api::btc::make_script_config_policy(policy, keys);

    // Register policy if not already registered. This must be done before any receive address is
    // created or any transaction is signed.
    let is_registered = paired_bitbox
        .btc_is_script_config_registered(coin, &policy_config, None)
        .await
        .unwrap();

    if !is_registered {
        paired_bitbox
            .btc_register_script_config(
                coin,
                &policy_config,
                None,
                pb::btc_register_script_config_request::XPubType::AutoXpubTpub,
                None,
            )
            .await
            .unwrap();
    }

    // Display receive address
    paired_bitbox
        .btc_address(
            coin,
            &"m/48'/1'/0'/3'/0/10".try_into().unwrap(),
            &policy_config,
            true,
        )
        .await
        .unwrap();
}
