// SPDX-License-Identifier: Apache-2.0

async fn demo<R: bitbox_api::runtime::Runtime + Sync + Send>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox = bitbox_api::BitBox::<R>::from_simulator(None, noise_config)
        .await
        .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    let paired_bitbox = match pairing_bitbox.wait_confirm().await.unwrap() {
        bitbox_api::PairedBitBox::Locked(device) => device.unlock().await.unwrap(),
        bitbox_api::PairedBitBox::Unlocked(device) => device,
        bitbox_api::PairedBitBox::Uninitialized(device) => {
            println!("device info: {:?}", device.device_info().await.unwrap());
            return;
        }
    };
    println!(
        "device info: {:?}",
        paired_bitbox.device_info().await.unwrap()
    );
}

#[tokio::main]
async fn main() {
    demo::<bitbox_api::runtime::TokioRuntime>().await
}
