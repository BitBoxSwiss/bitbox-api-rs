async fn demo<R: bitbox_api::runtime::Runtime>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox =
        bitbox_api::BitBox::<R>::from(bitbox_api::usb::get_any_bitbox02().unwrap(), noise_config)
            .await
            .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();
    println!(
        "root fingerprint: {}",
        paired_bitbox.root_fingerprint().await.unwrap()
    );

    paired_bitbox
        .btc_xpub(
            bitbox_api::pb::BtcCoin::Btc,
            &"m/84'/0'/0'".try_into().unwrap(),
            bitbox_api::pb::btc_pub_request::XPubType::Xpub,
            true,
        )
        .await
        .unwrap();
    paired_bitbox
        .btc_address(
            bitbox_api::pb::BtcCoin::Btc,
            &"m/84'/0'/0'/0/0".try_into().unwrap(),
            &bitbox_api::btc::make_script_config_simple(
                bitbox_api::pb::btc_script_config::SimpleType::P2wpkh,
            ),
            true,
        )
        .await
        .unwrap();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    demo::<bitbox_api::runtime::TokioRuntime>().await
}
