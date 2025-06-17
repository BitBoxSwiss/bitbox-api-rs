use bitbox_api::pb;

async fn signmsg<R: bitbox_api::runtime::Runtime>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox = bitbox_api::BitBox::<R>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02().unwrap(),
        noise_config,
    )
    .await
    .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{pairing_code}");
    }
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();

    let keypath = bitbox_api::Keypath::try_from("m/49'/0'/0'/0/0").unwrap();

    let script_config_sign_msg: pb::BtcScriptConfigWithKeypath = pb::BtcScriptConfigWithKeypath {
        script_config: Some(bitbox_api::btc::make_script_config_simple(
            pb::btc_script_config::SimpleType::P2wpkhP2sh,
        )),
        keypath: keypath.to_vec(),
    };

    let signature = paired_bitbox
        .btc_sign_message(pb::BtcCoin::Btc, script_config_sign_msg, b"message")
        .await
        .unwrap();
    println!("Signature: {signature:?}");
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    signmsg::<bitbox_api::runtime::TokioRuntime>().await
}
