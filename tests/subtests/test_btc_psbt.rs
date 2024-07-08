use super::PairedBitBox;

use bitbox_api::{btc::Xpub, pb};

use bitcoin::bip32::DerivationPath;
use bitcoin::psbt::Psbt;
use bitcoin::secp256k1;
use bitcoin::{
    transaction, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use miniscript::psbt::PsbtExt;

// Checks that the psbt is fully signed and valid (all scripts execute correctly).
//
// Alternatively, we could use `miniscript::psbt::interpreter_check(&psbt, &secp).unwrap();` to
// verify the tx (this can only verify spends of UTXOs created by a descriptor, but the BitBox does
// not support anything else). We stick to libbitcoinconsensus for now for verification (not
// miniscript based) while it is still around. libbitcoinconsensus is dead
// (https://github.com/bitcoin/bitcoin/blob/master/doc/release-notes/release-notes-27.0.md?plain=1#L40-L53)
// and will be replaced by libbitcoinkernel some day.
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
    test_policy_wsh(bitbox).await;
}

// Test signing; all inputs are BIP86 Taproot keyspends.
async fn test_taproot_key_spend(bitbox: &PairedBitBox) {
    let secp = secp256k1::Secp256k1::new();

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
    psbt.finalize_mut(&secp).unwrap();

    // Verify the signed tx, including that all sigs/witnesses are correct.
    verify_transaction(psbt);
}

// Test signing; mixed input types (p2wpkh, p2wpkh-p2sh, p2tr)
async fn test_mixed_spend(bitbox: &PairedBitBox) {
    let secp = secp256k1::Secp256k1::new();

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
    psbt.finalize_mut(&secp).unwrap();

    // Verify the signed tx, including that all sigs/witnesses are correct.
    verify_transaction(psbt);
}

async fn test_policy_wsh(bitbox: &PairedBitBox) {
    let secp = secp256k1::Secp256k1::new();

    let coin = pb::BtcCoin::Tbtc;
    // Policy string following BIP-388 syntax, input to the BitBox.
    let policy = "wsh(or_b(pk(@0/<0;1>/*),s:pk(@1/<0;1>/*)))";

    let our_root_fingerprint = super::simulator_xprv().fingerprint(&secp);

    let keypath_account: DerivationPath = "m/48'/1'/0'/3'".parse().unwrap();

    let our_xpub: Xpub = super::simulator_xpub_at(&secp, &keypath_account);
    let some_xpub: Xpub = "tpubDFgycCkexSxkdZfeyaasDHityE97kiYM1BeCNoivDHvydGugKtoNobt4vEX6YSHNPy2cqmWQHKjKxciJuocepsGPGxcDZVmiMBnxgA1JKQk".parse().unwrap();

    // We use the miniscript library to build a multipath descriptor including key origin so we can
    // easily derive the receive/change descriptor, pubkey scripts, populate the PSBT input key
    // infos and convert the sigs to final witnesses.

    let multi_descriptor: miniscript::Descriptor<miniscript::DescriptorPublicKey> = policy
        .replace(
            "@0",
            &format!("[{}/48'/1'/0'/3']{}", &our_root_fingerprint, &our_xpub),
        )
        .replace("@1", &some_xpub.to_string())
        .parse::<miniscript::Descriptor<miniscript::DescriptorPublicKey>>()
        .unwrap();
    assert!(multi_descriptor.sanity_check().is_ok());

    let [descriptor_receive, descriptor_change] = multi_descriptor
        .into_single_descriptors()
        .unwrap()
        .try_into()
        .unwrap();
    // Derive /0/0 (first receive) and /1/0 (first change) descriptors.
    let input_descriptor = descriptor_receive.at_derivation_index(0).unwrap();
    let change_descriptor = descriptor_change.at_derivation_index(0).unwrap();

    let keys = &[
        // Our key: root fingerprint and keypath are required.
        bitbox_api::btc::KeyOriginInfo {
            root_fingerprint: Some(our_root_fingerprint),
            keypath: Some((&keypath_account).into()),
            xpub: our_xpub,
        },
        // Foreign key: root fingerprint and keypath are optional.
        bitbox_api::btc::KeyOriginInfo {
            root_fingerprint: None,
            keypath: None,
            xpub: some_xpub,
        },
    ];
    let policy_config = bitbox_api::btc::make_script_config_policy(policy, keys);

    // Register policy if not already registered. This must be done before any receive address is
    // created or any transaction is signed.
    let is_registered = bitbox
        .btc_is_script_config_registered(coin, &policy_config, None)
        .await
        .unwrap();

    if !is_registered {
        bitbox
            .btc_register_script_config(
                coin,
                &policy_config,
                None,
                pb::btc_register_script_config_request::XPubType::AutoXpubTpub,
                Some("test wsh policy"),
            )
            .await
            .unwrap();
    }

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
        output: vec![TxOut {
            value: Amount::from_sat(100_000_000),
            script_pubkey: input_descriptor.script_pubkey(),
        }],
    };

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
                value: Amount::from_sat(70_000_000),
                script_pubkey: change_descriptor.script_pubkey(),
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
    // These add the input/output bip32_derivation entries / key infos.
    psbt.update_input_with_descriptor(0, &input_descriptor)
        .unwrap();
    psbt.update_output_with_descriptor(0, &change_descriptor)
        .unwrap();

    // Sign.
    bitbox
        .btc_sign_psbt(
            pb::BtcCoin::Tbtc,
            &mut psbt,
            Some(pb::BtcScriptConfigWithKeypath {
                script_config: Some(policy_config),
                keypath: keypath_account.to_u32_vec(),
            }),
            pb::btc_sign_init_request::FormatUnit::Default,
        )
        .await
        .unwrap();

    // Finalize, add witnesses.
    psbt.finalize_mut(&secp).unwrap();

    // Verify the signed tx, including that all sigs/witnesses are correct.
    verify_transaction(psbt);
}
