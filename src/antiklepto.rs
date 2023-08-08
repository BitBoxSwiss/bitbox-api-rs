use thiserror::Error;

use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{PublicKey, Scalar, Secp256k1};

use std::io::Write;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed generating antiklepto host nonce")]
    GenNonce,
    #[error("{0}")]
    VerificationErr(&'static str),
    #[error(
        "Could not verify that the host nonce was contributed to the signature. \
		 If this happens repeatedly, the device might be attempting to leak the \
		 seed through the signature."
    )]
    VerificationFailed,
}

fn tagged_sha256(tag: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut engine = sha256::Hash::engine();
    let tag_hash = sha256::Hash::hash(tag);

    engine.write_all(tag_hash.as_ref()).unwrap();
    engine.write_all(tag_hash.as_ref()).unwrap();
    engine.write_all(msg).unwrap();

    sha256::Hash::from_engine(engine).to_byte_array()
}

pub fn gen_host_nonce() -> Result<[u8; 32], Error> {
    let mut result = [0u8; 32];
    getrandom::getrandom(&mut result).map_err(|_| Error::GenNonce)?;
    Ok(result)
}

pub fn host_commit(host_nonce: &[u8]) -> [u8; 32] {
    tagged_sha256(b"s2c/ecdsa/data", host_nonce)
}

/// antikleptoVerify verifies that hostNonce was used to tweak the nonce during signature
/// generation according to k' = k + H(clientCommitment, hostNonce) by checking that
/// k'*G = signerCommitment + H(signerCommitment, hostNonce)*G.
pub fn verify_ecdsa(
    host_nonce: &[u8],
    signer_commitment: &[u8],
    signature: &[u8],
) -> Result<(), Error> {
    let secp = Secp256k1::new();
    let signer_commitment_pubkey = PublicKey::from_slice(signer_commitment)
        .map_err(|_| Error::VerificationErr("Failed to parse public key"))?;

    // Compute R = R1 + H(R1, host_nonce)*G.
    let mut data = signer_commitment_pubkey.serialize().to_vec();
    data.extend_from_slice(host_nonce);

    let tweak = tagged_sha256(b"s2c/ecdsa/point", &data);

    let tweaked_point = signer_commitment_pubkey
        .add_exp_tweak(
            &secp,
            &Scalar::from_be_bytes(tweak)
                .map_err(|_| Error::VerificationErr("tweak is an invalid scalar"))?,
        )
        .map_err(|_| Error::VerificationErr("Failed to tweak key"))?;

    let x_coordinate = &tweaked_point.serialize()[1..33];
    let signature_r = &signature[..32];
    if x_coordinate != signature_r {
        return Err(Error::VerificationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::hex::FromHex;

    #[test]
    fn test_tagged_sha256() {
        let expected_hash: [u8; 32] =
            FromHex::from_hex("025ee06f5a2db377bd9d7040bae8f6e0ab49784f9c68a1380fba5465d8a99928")
                .unwrap();
        assert_eq!(expected_hash, tagged_sha256(b"test tag", b"test message"));
    }

    #[test]
    fn test_host_commit() {
        let host_nonce: [u8; 32] =
            FromHex::from_hex("e8011345fe4851538c30c1fc1a215395e8063fcf6fbdcf8fab9a42e466a74f4a")
                .unwrap();
        let expected_hash: [u8; 32] =
            FromHex::from_hex("70a8934f41a1679b4c715c3e6db17f785b67da4e398107a0a00c828980a4be2f")
                .unwrap();

        assert_eq!(expected_hash, host_commit(&host_nonce));
    }

    #[test]
    fn test_verify_ecdsa() {
        struct TestVector {
            host_nonce: Vec<u8>,
            signer_commitment: Vec<u8>,
            signature: Vec<u8>,
        }
        let unhex = |s| FromHex::from_hex(s).unwrap();

        // Fixtures made by running the protocol and recording the values.
        let test_cases = [
            TestVector {
                host_nonce: unhex("8b4c26aa2695a34bdbc34235f6c91be14b93037a063b13f7c814101359561092"),
                signer_commitment: unhex("0236ff92fe02c08d0d04851e0ce1516104085215f05a178307de60ea53e207f971"),
                signature: unhex("7fd66b48ffea2fe048869880bbb3a1819e262af14980e8885df1e5765750cb8f47e01eca356377870356d54853573a955076228e5044cd3dd3a049abe70d5585"),
            },
            TestVector {
                host_nonce: unhex("9c9471aa529fbad96396b9379938e56195c5aa8e1e22b6e87d226e49d8b1f581"),
                signer_commitment: unhex("034e2979d398ce029996ffe99dc310a0f2cf9a5411b166f57a85fc3a24985f16be"),
                signature: unhex("48cb61d08c730e36b0285dfd9ece91e88a5ec0898d1c80b93e85b967e0ddcd195ab807640347e8f96e3fad67a971fc52eb4f15b4fa65577bcf4a053e598d057d"),
            }
        ];

        for mut test_case in test_cases.into_iter() {
            assert!(verify_ecdsa(
                &test_case.host_nonce,
                &test_case.signer_commitment,
                &test_case.signature
            )
            .is_ok());

            // Tweak an input a bit to fail verification.
            test_case.host_nonce[0] += 1;
            assert!(verify_ecdsa(
                &test_case.host_nonce,
                &test_case.signer_commitment,
                &test_case.signature
            )
            .is_err());
        }
    }
}
