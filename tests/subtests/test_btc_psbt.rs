use super::PairedBitBox;

use bitbox_api::pb;

use bitcoin::bip32::DerivationPath;
use bitcoin::psbt::Psbt;
use bitcoin::{
    transaction, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};

// Checks that the psbt is fully signed and valid (all scripts execute correctly).
fn verify_transaction(psbt: Psbt) {
    let utxos: Vec<TxOut> = psbt
        .iter_funding_utxos()
        .map(|utxo| utxo.unwrap())
        .cloned()
        .collect();

    let tx = psbt.extract_tx_unchecked_fee_rate();
    let serialized_tx = bitcoin::consensus::encode::serialize(&tx);

    let flags = bitcoinconsensus::VERIFY_ALL_PRE_TAPROOT | bitcoinconsensus::VERIFY_TAPROOT;

    let utxos_converted: Vec<bitcoinconsensus::Utxo> = utxos
        .iter()
        .map(|output| bitcoinconsensus::Utxo {
            script_pubkey: output.script_pubkey.as_bytes().as_ptr(),
            script_pubkey_len: output.script_pubkey.as_bytes().len() as u32,
            value: output.value.to_sat() as i64,
        })
        .collect();

    for (idx, output) in utxos.iter().enumerate() {
        bitcoinconsensus::verify_with_flags(
            output.script_pubkey.as_bytes(),
            output.value.to_sat(),
            serialized_tx.as_slice(),
            Some(&utxos_converted),
            idx,
            flags,
        )
        .unwrap();
    }
}

pub async fn test(bitbox: &PairedBitBox) {
    test_taproot_key_spend(bitbox).await;
    test_mixed_spend(bitbox).await;
}

// Test signing; all inputs are BIP86 Taproot keyspends.
async fn test_taproot_key_spend(bitbox: &PairedBitBox) {
    let secp = bitcoin::secp256k1::Secp256k1::new();

    let fingerprint = super::simulator_xprv().fingerprint(&secp);

    let change_path: DerivationPath = "m/86'/1'/0'/1/0".parse().unwrap();
    let change_xpub = super::simulator_xpub_at(&secp, &change_path);

    let input0_path: DerivationPath = "m/86'/1'/0'/0/0".parse().unwrap();
    let input0_xpub = super::simulator_xpub_at(&secp, &input0_path);

    let input1_path: DerivationPath = "m/86'/1'/0'/0/1".parse().unwrap();
    let input1_xpub = super::simulator_xpub_at(&secp, &input1_path);

    // A previous tx which creates some UTXOs we can reference later.
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
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2tr(&secp, input0_xpub.to_x_only_pub(), None),
            },
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2tr(&secp, input1_xpub.to_x_only_pub(), None),
            },
        ],
    };

    let tx = Transaction {
        version: transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![
            TxIn {
                previous_output: OutPoint {
                    txid: prev_tx.compute_txid(),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            },
            TxIn {
                previous_output: OutPoint {
                    txid: prev_tx.compute_txid(),
                    vout: 1,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            },
        ],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2tr(&secp, change_xpub.to_x_only_pub(), None),
            },
            TxOut {
                value: Amount::from_sat(20_000_000),
                script_pubkey: ScriptBuf::new_p2tr(
                    &secp,
                    // random private key:
                    // 9dbb534622a6100a39b73dece43c6d4db14b9a612eb46a6c64c2bb849e283ce8
                    "e4adbb12c3426ec71ebb10688d8ae69d531ca822a2b790acee216a7f1b95b576"
                        .parse()
                        .unwrap(),
                    None,
                ),
            },
        ],
    };

    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();

    // Add input and change infos.
    psbt.inputs[0].witness_utxo = Some(prev_tx.output[0].clone());
    psbt.inputs[0].tap_internal_key = Some(input0_xpub.to_x_only_pub());
    psbt.inputs[0].tap_key_origins.insert(
        input0_xpub.to_x_only_pub(),
        (vec![], (fingerprint, input0_path.clone())),
    );
    psbt.inputs[1].witness_utxo = Some(prev_tx.output[1].clone());
    psbt.inputs[1].tap_internal_key = Some(input1_xpub.to_x_only_pub());
    psbt.inputs[1].tap_key_origins.insert(
        input1_xpub.to_x_only_pub(),
        (vec![], (fingerprint, input1_path.clone())),
    );

    psbt.outputs[0].tap_internal_key = Some(change_xpub.to_x_only_pub());
    psbt.outputs[0].tap_key_origins.insert(
        change_xpub.to_x_only_pub(),
        (vec![], (fingerprint, change_path.clone())),
    );

    // Sign.
    bitbox
        .btc_sign_psbt(
            pb::BtcCoin::Tbtc,
            &mut psbt,
            None,
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();

    // Finalize, add witnesses.
    psbt.inputs.iter_mut().for_each(|input| {
        let mut script_witness = Witness::new();
        script_witness.push(input.tap_key_sig.unwrap().to_vec());
        input.final_script_witness = Some(script_witness);
    });

    // Verify the signed tx, including that all sigs/witnesses are correct.
    verify_transaction(psbt);
}

// Test signing; mixed input types (p2wpkh, p2wpkh-p2sh, p2tr)
async fn test_mixed_spend(bitbox: &PairedBitBox) {
    let secp = bitcoin::secp256k1::Secp256k1::new();

    let fingerprint = super::simulator_xprv().fingerprint(&secp);

    let change_path: DerivationPath = "m/86'/1'/0'/1/0".parse().unwrap();
    let change_xpub = super::simulator_xpub_at(&secp, &change_path);

    let input0_path: DerivationPath = "m/86'/1'/0'/0/0".parse().unwrap();
    let input0_xpub = super::simulator_xpub_at(&secp, &input0_path);

    let input1_path: DerivationPath = "m/84'/1'/0'/0/0".parse().unwrap();
    let input1_xpub = super::simulator_xpub_at(&secp, &input1_path);

    let input2_path: DerivationPath = "m/49'/1'/0'/0/0".parse().unwrap();
    let input2_xpub = super::simulator_xpub_at(&secp, &input2_path);

    let input2_redeemscript = ScriptBuf::new_p2wpkh(&input2_xpub.to_pub().wpubkey_hash());

    // A previous tx which creates some UTXOs we can reference later.
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
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2tr(&secp, input0_xpub.to_x_only_pub(), None),
            },
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2wpkh(&input1_xpub.to_pub().wpubkey_hash()),
            },
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2sh(&input2_redeemscript.clone().into()),
            },
        ],
    };

    let tx = Transaction {
        version: transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![
            TxIn {
                previous_output: OutPoint {
                    txid: prev_tx.compute_txid(),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            },
            TxIn {
                previous_output: OutPoint {
                    txid: prev_tx.compute_txid(),
                    vout: 1,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            },
            TxIn {
                previous_output: OutPoint {
                    txid: prev_tx.compute_txid(),
                    vout: 2,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            },
        ],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: ScriptBuf::new_p2tr(&secp, change_xpub.to_x_only_pub(), None),
            },
            TxOut {
                value: Amount::from_sat(20_000_000),
                script_pubkey: ScriptBuf::new_p2tr(
                    &secp,
                    // random private key:
                    // 9dbb534622a6100a39b73dece43c6d4db14b9a612eb46a6c64c2bb849e283ce8
                    "e4adbb12c3426ec71ebb10688d8ae69d531ca822a2b790acee216a7f1b95b576"
                        .parse()
                        .unwrap(),
                    None,
                ),
            },
        ],
    };

    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();

    // Add input and change infos.
    psbt.inputs[0].non_witness_utxo = Some(prev_tx.clone());
    psbt.inputs[0].tap_internal_key = Some(input0_xpub.to_x_only_pub());
    psbt.inputs[0].tap_key_origins.insert(
        input0_xpub.to_x_only_pub(),
        (vec![], (fingerprint, input0_path.clone())),
    );

    psbt.inputs[1].non_witness_utxo = Some(prev_tx.clone());
    psbt.inputs[1]
        .bip32_derivation
        .insert(input1_xpub.to_pub().0, (fingerprint, input1_path.clone()));

    psbt.inputs[2].non_witness_utxo = Some(prev_tx.clone());
    psbt.inputs[2].redeem_script = Some(input2_redeemscript.clone());
    psbt.inputs[2]
        .bip32_derivation
        .insert(input2_xpub.to_pub().0, (fingerprint, input2_path.clone()));

    psbt.outputs[0].tap_internal_key = Some(change_xpub.to_x_only_pub());
    psbt.outputs[0].tap_key_origins.insert(
        change_xpub.to_x_only_pub(),
        (vec![], (fingerprint, change_path.clone())),
    );

    // Sign.
    bitbox
        .btc_sign_psbt(
            pb::BtcCoin::Tbtc,
            &mut psbt,
            None,
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();

    // Finalize, add witnesses.

    // p2tr
    psbt.inputs[0].final_script_witness = Some(Witness::p2tr_key_spend(
        psbt.inputs[0].tap_key_sig.as_ref().unwrap(),
    ));
    // p2wpkh
    psbt.inputs[1].final_script_witness = Some({
        let (pubkey, sig) = psbt.inputs[1].partial_sigs.first_key_value().unwrap();
        Witness::p2wpkh(sig, &pubkey.inner)
    });
    // p2wpkh-p2sh needs a witness (for the p2wpkh part) and a script_sig (for the p2sh part).
    psbt.inputs[2].final_script_sig = Some({
        let redeemscript: &bitcoin::script::PushBytes =
            input2_redeemscript.as_bytes().try_into().unwrap();
        let mut script = ScriptBuf::new();
        script.push_slice(redeemscript);
        script
    });
    psbt.inputs[2].final_script_witness = Some({
        let (pubkey, sig) = psbt.inputs[2].partial_sigs.first_key_value().unwrap();
        Witness::p2wpkh(sig, &pubkey.inner)
    });

    // Verify the signed tx, including that all sigs/witnesses are correct.
    verify_transaction(psbt);
}
