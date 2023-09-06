fn multithreading_type_check<T: Sync + Send>(_t: &T) {}

async fn demo<R: bitbox_api::runtime::Runtime + Sync + Send>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox =
        bitbox_api::BitBox::<R>::from(bitbox_api::usb::get_any_bitbox02().unwrap(), noise_config)
            .await
            .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    multithreading_type_check(&pairing_bitbox);
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();
    println!(
        "root fingerprint: {}",
        paired_bitbox.root_fingerprint().await.unwrap()
    );
    multithreading_type_check(&paired_bitbox);
}

#[tokio::main]
async fn main() {
    demo::<bitbox_api::runtime::TokioRuntime>().await
}
