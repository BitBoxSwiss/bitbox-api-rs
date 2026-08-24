// SPDX-License-Identifier: Apache-2.0

use bitcoin::secp256k1::ecdsa::Signature;
use thiserror::Error;

/// An invalid ECDSA signature encoding.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ValidationError(&'static str);

/// Validates a 64-byte compact ECDSA signature encoded as `r || s`.
///
/// Both scalars must be nonzero and in range, and `s` must use its low-S encoding.
pub(crate) fn validate_signature_compact(signature: &[u8]) -> Result<(), ValidationError> {
    if signature.len() != 64 {
        return Err(ValidationError("Signature must be 64 bytes"));
    }

    let parsed_signature = Signature::from_compact(signature)
        .map_err(|_| ValidationError("Failed to parse ECDSA signature"))?;
    if signature[..32].iter().all(|byte| *byte == 0)
        || signature[32..].iter().all(|byte| *byte == 0)
    {
        return Err(ValidationError("Invalid ECDSA signature"));
    }
    let mut normalized_signature = parsed_signature;
    normalized_signature.normalize_s();
    if normalized_signature != parsed_signature {
        return Err(ValidationError("ECDSA signature has high S"));
    }
    Ok(())
}

/// Validates a 65-byte recoverable ECDSA signature encoded as `r || s || recovery_id`.
///
/// The compact signature must be valid and the recovery ID must be in the range 0..=3.
pub(crate) fn validate_signature_recoverable(signature: &[u8]) -> Result<(), ValidationError> {
    if signature.len() != 65 {
        return Err(ValidationError("Signature must be 65 bytes"));
    }
    validate_signature_compact(&signature[..64])?;
    if signature[64] > 3 {
        return Err(ValidationError("Invalid recovery ID"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::hex::FromHex;

    fn valid_signature() -> Vec<u8> {
        FromHex::from_hex(
            "7fd66b48ffea2fe048869880bbb3a1819e262af14980e8885df1e5765750cb8f47e01eca356377870356d54853573a955076228e5044cd3dd3a049abe70d5585",
        )
        .unwrap()
    }

    #[test]
    fn test_validate_signature_compact() {
        let signature = valid_signature();
        assert!(validate_signature_compact(&signature).is_ok());

        let high_s: Vec<u8> = FromHex::from_hex(
            "7fd66b48ffea2fe048869880bbb3a1819e262af14980e8885df1e5765750cb8fb81fe135ca9c8878fca92ab7aca8c5696a38ba585f03d2fdec3214e0e928ebbc",
        )
        .unwrap();
        assert!(Signature::from_compact(&high_s).is_ok());
        assert!(validate_signature_compact(&high_s).is_err());

        assert!(validate_signature_compact(&signature[..63]).is_err());
        let mut too_long = signature.clone();
        too_long.push(0);
        assert!(validate_signature_compact(&too_long).is_err());

        for offset in [0, 32] {
            let mut zero = signature.clone();
            zero[offset..offset + 32].fill(0);
            assert!(validate_signature_compact(&zero).is_err());

            let mut out_of_range = signature.clone();
            out_of_range[offset..offset + 32]
                .copy_from_slice(&bitcoin::secp256k1::constants::CURVE_ORDER);
            assert!(validate_signature_compact(&out_of_range).is_err());
        }
    }

    #[test]
    fn test_validate_signature_recoverable() {
        let mut signature = valid_signature();
        signature.push(0);
        assert!(validate_signature_recoverable(&signature).is_ok());
        assert!(validate_signature_recoverable(&signature[..64]).is_err());

        signature[64] = 3;
        assert!(validate_signature_recoverable(&signature).is_ok());

        signature[64] = 4;
        assert!(validate_signature_recoverable(&signature).is_err());
    }
}
