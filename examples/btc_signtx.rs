use bitbox_api::pb;

async fn signtx<R: bitbox_api::runtime::Runtime>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox =
        bitbox_api::BitBox::<R, _>::from(bitbox_api::usb::get_any_bitbox02().unwrap(), noise_config)
            .await
            .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();

    let prevtx = bitbox_api::btc::PrevTx {
        version: 1,
        inputs: vec![bitbox_api::btc::PrevTxInput {
            prev_out_hash: vec![b'1'; 32],
            prev_out_index: 0,
            signature_script: b"some signature script".to_vec(),
            sequence: 0xFFFFFFFF,
        }],
        outputs: vec![bitbox_api::btc::PrevTxOutput {
            value: 60005000,
            pubkey_script: b"some pubkey script".to_vec(),
        }],
        locktime: 0,
    };
    let transaction = bitbox_api::btc::Transaction {
        script_configs: vec![
            pb::BtcScriptConfigWithKeypath {
                script_config: Some(bitbox_api::btc::make_script_config_simple(
                    pb::btc_script_config::SimpleType::P2wpkh,
                )),
                keypath: bitbox_api::Keypath::try_from("m/84'/0'/0'")
                    .unwrap()
                    .to_vec(),
            },
            pb::BtcScriptConfigWithKeypath {
                script_config: Some(bitbox_api::btc::make_script_config_simple(
                    pb::btc_script_config::SimpleType::P2wpkhP2sh,
                )),
                keypath: bitbox_api::Keypath::try_from("m/49'/0'/0'")
                    .unwrap()
                    .to_vec(),
            },
        ],
        version: 1,
        inputs: vec![
            bitbox_api::btc::TxInput {
                prev_out_hash: hex::decode(
                    "c58b7e3f1200e0c0ec9a5e81e925baface2cc1d4715514f2d8205be2508b48ee",
                )
                .unwrap(),
                prev_out_index: 0,
                prev_out_value: 60005000,
                sequence: 0xFFFFFFFF,
                keypath: "m/84'/0'/0'/0/0".try_into().unwrap(),
                script_config_index: 0,
                prev_tx: Some(prevtx.clone()),
            },
            bitbox_api::btc::TxInput {
                prev_out_hash: hex::decode(
                    "c58b7e3f1200e0c0ec9a5e81e925baface2cc1d4715514f2d8205be2508b48ee",
                )
                .unwrap(),
                prev_out_index: 0,
                prev_out_value: 60005000,
                sequence: 0xFFFFFFFF,
                keypath: "m/49'/0'/0'/0/1".try_into().unwrap(),
                script_config_index: 1,
                prev_tx: Some(prevtx.clone()),
            },
        ],
        outputs: vec![
            bitbox_api::btc::TxOutput::Internal(bitbox_api::btc::TxInternalOutput {
                keypath: "m/84'/0'/0'/1/0".try_into().unwrap(),
                value: 100000000,
                script_config_index: 0,
            }),
            bitbox_api::btc::TxOutput::External(bitbox_api::btc::TxExternalOutput {
                payload: bitbox_api::btc::Payload {
                    data: vec![1; 32],
                    output_type: pb::BtcOutputType::P2wsh,
                },
                value: 20000000,
            }),
        ],
        locktime: 0,
    };
    let sigs = paired_bitbox
        .btc_sign(
            pb::BtcCoin::Btc,
            &transaction,
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();
    println!("Sigs:");
    for (i, sig) in sigs.iter().enumerate() {
        println!("Input {}: {}", i, hex::encode(sig));
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    signtx::<bitbox_api::runtime::TokioRuntime>().await
}
