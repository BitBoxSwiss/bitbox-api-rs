// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod util;

use bitbox_api::eth::{EIP1559Transaction, Transaction};
use bitcoin::secp256k1;
use tiny_keccak::{Hasher, Keccak};
use util::test_initialized_simulators;

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

fn legacy_sighash(chain_id: u64, tx: &Transaction) -> [u8; 32] {
    let mut stream = rlp::RlpStream::new_list(9);
    stream.append(&tx.nonce);
    stream.append(&tx.gas_price);
    stream.append(&tx.gas_limit);
    let recipient = tx.recipient.to_vec();
    stream.append(&recipient);
    stream.append(&tx.value);
    stream.append(&tx.data);
    stream.append(&chain_id);
    stream.append(&0u64);
    stream.append(&0u64);
    keccak256(&stream.out())
}

fn eip1559_sighash(tx: &EIP1559Transaction) -> [u8; 32] {
    let mut stream = rlp::RlpStream::new_list(9);
    stream.append(&tx.chain_id);
    stream.append(&tx.nonce);
    stream.append(&tx.max_priority_fee_per_gas);
    stream.append(&tx.max_fee_per_gas);
    stream.append(&tx.gas_limit);
    let recipient = tx.recipient.to_vec();
    stream.append(&recipient);
    stream.append(&tx.value);
    stream.append(&tx.data);
    stream.begin_list(0);
    let rlp_bytes = stream.out();
    let mut prefixed = vec![0x02];
    prefixed.extend_from_slice(&rlp_bytes);
    keccak256(&prefixed)
}

fn verify_eth_signature(sighash: &[u8; 32], signature: &[u8; 65]) {
    let secp = secp256k1::Secp256k1::new();
    let path: bitcoin::bip32::DerivationPath = "m/44'/60'/0'/0/0".parse().unwrap();
    let child_xprv = util::simulator_xprv().derive_priv(&secp, &path).unwrap();
    let expected_pubkey = bitcoin::bip32::Xpub::from_priv(&secp, &child_xprv).public_key;

    let sig = secp256k1::ecdsa::Signature::from_compact(&signature[..64]).unwrap();
    let msg = secp256k1::Message::from_digest(*sighash);

    secp.verify_ecdsa(&msg, &sig, &expected_pubkey).unwrap();
}

#[tokio::test]
async fn test_eth_address() {
    test_initialized_simulators(async |paired_bitbox| {
        let address = paired_bitbox
            .eth_address(1, &"m/44'/60'/0'/0/0".try_into().unwrap(), false)
            .await
            .unwrap();
        assert_eq!(address, "0x416E88840Eb6353E49252Da2a2c140eA1f969D1a");
    })
    .await
}

#[tokio::test]
async fn test_eth_sign_transaction_nonstreaming() {
    test_initialized_simulators(async |paired_bitbox| {
        assert!(paired_bitbox.eth_supported());

        let tx = Transaction {
            nonce: vec![0x01],
            gas_price: vec![0x01],
            gas_limit: vec![0x52, 0x08],
            recipient: [
                0x04, 0xf2, 0x64, 0xcf, 0x34, 0x44, 0x03, 0x13, 0xb4, 0xa0, 0x19, 0x2a, 0x35, 0x28,
                0x14, 0xfb, 0xe9, 0x27, 0xb8, 0x85,
            ],
            value: vec![0x01],
            data: vec![0xAB; 100],
        };

        let signature = paired_bitbox
            .eth_sign_transaction(1, &"m/44'/60'/0'/0/0".try_into().unwrap(), &tx, None)
            .await
            .unwrap();
        assert_eq!(signature.len(), 65);
        verify_eth_signature(&legacy_sighash(1, &tx), &signature);
    })
    .await
}

#[tokio::test]
async fn test_eth_sign_transaction_streaming() {
    test_initialized_simulators(async |paired_bitbox| {
        if !semver::VersionReq::parse(">=9.26.0")
            .unwrap()
            .matches(paired_bitbox.version())
        {
            return;
        }

        // Large data (over threshold) - streaming mode
        let tx = Transaction {
            nonce: vec![0x01],
            gas_price: vec![0x01],
            gas_limit: vec![0x52, 0x08],
            recipient: [
                0x04, 0xf2, 0x64, 0xcf, 0x34, 0x44, 0x03, 0x13, 0xb4, 0xa0, 0x19, 0x2a, 0x35, 0x28,
                0x14, 0xfb, 0xe9, 0x27, 0xb8, 0x85,
            ],
            value: vec![0x01],
            data: vec![0xAB; 10000],
        };

        let signature = paired_bitbox
            .eth_sign_transaction(1, &"m/44'/60'/0'/0/0".try_into().unwrap(), &tx, None)
            .await
            .unwrap();
        assert_eq!(signature.len(), 65);
        verify_eth_signature(&legacy_sighash(1, &tx), &signature);
    })
    .await
}

#[tokio::test]
async fn test_eth_sign_1559_transaction_nonstreaming() {
    test_initialized_simulators(async |paired_bitbox| {
        assert!(paired_bitbox.eth_supported());

        let tx = EIP1559Transaction {
            chain_id: 1,
            nonce: vec![0x01],
            max_priority_fee_per_gas: vec![0x01],
            max_fee_per_gas: vec![0x01],
            gas_limit: vec![0x52, 0x08],
            recipient: [
                0x04, 0xf2, 0x64, 0xcf, 0x34, 0x44, 0x03, 0x13, 0xb4, 0xa0, 0x19, 0x2a, 0x35, 0x28,
                0x14, 0xfb, 0xe9, 0x27, 0xb8, 0x85,
            ],
            value: vec![0x01],
            data: vec![0xAB; 100],
        };

        let signature = paired_bitbox
            .eth_sign_1559_transaction(&"m/44'/60'/0'/0/0".try_into().unwrap(), &tx, None)
            .await
            .unwrap();
        assert_eq!(signature.len(), 65);
        verify_eth_signature(&eip1559_sighash(&tx), &signature);
    })
    .await
}

#[tokio::test]
async fn test_eth_sign_1559_transaction_streaming() {
    test_initialized_simulators(async |paired_bitbox| {
        if !semver::VersionReq::parse(">=9.26.0")
            .unwrap()
            .matches(paired_bitbox.version())
        {
            return;
        }

        let tx = EIP1559Transaction {
            chain_id: 1,
            nonce: vec![0x01],
            max_priority_fee_per_gas: vec![0x01],
            max_fee_per_gas: vec![0x01],
            gas_limit: vec![0x52, 0x08],
            recipient: [
                0x04, 0xf2, 0x64, 0xcf, 0x34, 0x44, 0x03, 0x13, 0xb4, 0xa0, 0x19, 0x2a, 0x35, 0x28,
                0x14, 0xfb, 0xe9, 0x27, 0xb8, 0x85,
            ],
            value: vec![0x01],
            data: vec![0xCD; 8000],
        };

        let signature = paired_bitbox
            .eth_sign_1559_transaction(&"m/44'/60'/0'/0/0".try_into().unwrap(), &tx, None)
            .await
            .unwrap();
        assert_eq!(signature.len(), 65);
        verify_eth_signature(&eip1559_sighash(&tx), &signature);
    })
    .await
}
