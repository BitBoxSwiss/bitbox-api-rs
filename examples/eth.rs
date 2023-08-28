async fn eth_demo<R: bitbox_api::runtime::Runtime>() {
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

    println!("Getting xpub...");
    let xpub = paired_bitbox
        .eth_xpub(&"m/44'/60'/0'/0".try_into().unwrap())
        .await
        .unwrap();
    println!("Xpub: {}", xpub);

    println!("Verifying address...");
    let address = paired_bitbox
        .eth_address(1, &"m/44'/60'/0'/0/0".try_into().unwrap(), true)
        .await
        .unwrap();
    println!("Address: {}", address);

    println!("Signing a tx...");
    let raw_tx = hex::decode("f86e821fdc850165a0bc008252089404f264cf34440313b4a0192a352814fbe927b88588075cf1259e9c40008025a015c94c1a3da0abc0a9124d2837809ccc493c41504e4571bcc340eeb68a91f641a03599011d4cda2c33dd3b00071ec145335e5d2dd5ed812d5eebeecba5264ed1bf").unwrap();
    let signature = paired_bitbox
        .eth_sign_transaction(
            1,
            &"m/44'/60'/0'/0/0".try_into().unwrap(),
            &raw_tx.as_slice().try_into().unwrap(),
        )
        .await
        .unwrap();
    println!("Signature: {}", hex::encode(signature));

    println!("Signing a message...");
    let signature = paired_bitbox
        .eth_sign_message(1, &"m/44'/60'/0'/0/0".try_into().unwrap(), b"message")
        .await
        .unwrap();
    println!("Signature: {}", hex::encode(signature));
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    eth_demo::<bitbox_api::runtime::TokioRuntime>().await
}
