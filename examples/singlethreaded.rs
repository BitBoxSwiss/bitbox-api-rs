// SPDX-License-Identifier: Apache-2.0

async fn demo<R: bitbox_api::runtime::Runtime>() {
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
    let paired_bitbox = match pairing_bitbox.wait_confirm().await.unwrap() {
        bitbox_api::PairedBitBox::Locked(device) => device
            .unlock(bitbox_api::PassphraseConfig::default())
            .await
            .unwrap(),
        bitbox_api::PairedBitBox::Unlocked(device) => device,
        bitbox_api::PairedBitBox::Uninitialized(_) => panic!("Initialize the BitBox first"),
    };
    println!(
        "root fingerprint: {}",
        paired_bitbox.root_fingerprint().await.unwrap()
    );
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    demo::<bitbox_api::runtime::TokioRuntime>().await
}
