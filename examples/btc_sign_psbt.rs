use bitbox_api::pb;
use std::str::FromStr;

async fn sign_psbt<R: bitbox_api::runtime::Runtime>(
    psbt: &mut bitcoin::psbt::PartiallySignedTransaction,
) {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let d = bitbox_api::BitBox::<R>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02().unwrap(),
        noise_config,
    )
    .await
    .unwrap();
    let pairing_device = d.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_device.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    let paired = pairing_device.wait_confirm().await.unwrap();
    paired
        .btc_sign_psbt(
            pb::BtcCoin::Tbtc,
            psbt,
            None,
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Paste a Bitcoin testnet PSBT in base64 format on one line and hit enter");
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer).unwrap();
    let mut psbt = bitcoin::psbt::PartiallySignedTransaction::from_str(buffer.trim()).unwrap();
    sign_psbt::<bitbox_api::runtime::TokioRuntime>(&mut psbt).await;
    println!("signed:");
    println!("{}", psbt);
}
