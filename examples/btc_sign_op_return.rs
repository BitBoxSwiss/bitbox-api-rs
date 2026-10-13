// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use bitbox_api::{pb, Keypath};

use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Fingerprint, Xpub},
    blockdata::script::Builder,
    opcodes::all,
    psbt::Psbt,
    secp256k1, transaction, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Witness,
};
use semver::VersionReq;

async fn connect_bitbox() -> bitbox_api::UnlockedBitBox<bitbox_api::runtime::TokioRuntime> {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let device = bitbox_api::BitBox::<bitbox_api::runtime::TokioRuntime>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02().unwrap(),
        noise_config,
    )
    .await
    .unwrap();
    let pairing = device.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing.get_pairing_code().as_ref() {
        println!("Pairing code\n{pairing_code}");
    }
    match pairing.wait_confirm().await.unwrap() {
        bitbox_api::PairedBitBox::Locked(device) => device
            .unlock(bitbox_api::PassphraseConfig::default())
            .await
            .unwrap(),
        bitbox_api::PairedBitBox::Unlocked(device) => device,
        bitbox_api::PairedBitBox::Uninitialized(_) => panic!("Initialize the BitBox first"),
    }
}

async fn build_op_return_psbt(
    paired: &bitbox_api::UnlockedBitBox<bitbox_api::runtime::TokioRuntime>,
) -> Psbt {
    let coin = pb::BtcCoin::Tbtc;
    let secp = secp256k1::Secp256k1::new();

    let fingerprint_hex = paired.root_fingerprint().await.unwrap();
    let fingerprint = Fingerprint::from_str(&fingerprint_hex).unwrap();
    let account_path: DerivationPath = "m/84'/1'/0'".parse().unwrap();
    let input_path: DerivationPath = "m/84'/1'/0'/0/5".parse().unwrap();
    let change_path: DerivationPath = "m/84'/1'/0'/1/0".parse().unwrap();

    let account_keypath = Keypath::from(&account_path);
    let account_xpub = paired
        .btc_xpub(
            coin,
            &account_keypath,
            pb::btc_pub_request::XPubType::Tpub,
            false,
        )
        .await
        .unwrap();

    let account_xpub = Xpub::from_str(&account_xpub).unwrap();

    let input_pub = account_xpub
        .derive_pub(
            &secp,
            &[
                ChildNumber::from_normal_idx(0).unwrap(),
                ChildNumber::from_normal_idx(5).unwrap(),
            ],
        )
        .unwrap()
        .to_pub();
    let change_pub = account_xpub
        .derive_pub(
            &secp,
            &[
                ChildNumber::from_normal_idx(1).unwrap(),
                ChildNumber::from_normal_idx(0).unwrap(),
            ],
        )
        .unwrap()
        .to_pub();

    let prev_tx = Transaction {
        version: transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: "3131313131313131313131313131313131313131313131313131313131313131:0"
                .parse()
                .unwrap(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence(0xFFFFFFFF),
            witness: Witness::default(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(50_000_000),
            script_pubkey: ScriptBuf::new_p2wpkh(&input_pub.wpubkey_hash()),
        }],
    };

    let op_return_data = b"hello world";
    let op_return_script = Builder::new()
        .push_opcode(all::OP_RETURN)
        .push_slice(op_return_data)
        .into_script();

    let tx = Transaction {
        version: transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: prev_tx.compute_txid(),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence(0xFFFFFFFF),
            witness: Witness::default(),
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(49_000_000),
                script_pubkey: ScriptBuf::new_p2wpkh(&change_pub.wpubkey_hash()),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: op_return_script,
            },
        ],
    };

    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    psbt.inputs[0].non_witness_utxo = Some(prev_tx.clone());
    psbt.inputs[0].witness_utxo = Some(prev_tx.output[0].clone());
    psbt.inputs[0]
        .bip32_derivation
        .insert(input_pub.0, (fingerprint, input_path.clone()));

    psbt.outputs[0]
        .bip32_derivation
        .insert(change_pub.0, (fingerprint, change_path.clone()));

    psbt
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let paired = connect_bitbox().await;

    let firmware_version = paired.version();
    if !VersionReq::parse(">=9.24.0")
        .unwrap()
        .matches(firmware_version)
    {
        eprintln!(
            "Connected firmware {firmware_version} does not support OP_RETURN outputs (requires >=9.24.0)."
        );
        return;
    }

    let mut psbt = build_op_return_psbt(&paired).await;

    paired
        .btc_sign_psbt(
            pb::BtcCoin::Tbtc,
            &mut psbt,
            None,
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();
}
