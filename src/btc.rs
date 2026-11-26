// SPDX-License-Identifier: Apache-2.0

//! Functions and methods related to Bitcoin.

#[cfg(feature = "client")]
use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb;
use crate::pb::{request::Request, response::Response};
use crate::Keypath;
#[cfg(feature = "client")]
use crate::PairedBitBox;

pub use bitcoin::{
    bip32::{Fingerprint, Xpub},
    blockdata::script::witness_version::WitnessVersion,
    Script,
};

use bitcoin::blockdata::{opcodes, script::Instruction};
use bitcoin::psbt::SignError;

#[derive(Clone, Debug, PartialEq)]
pub struct PrevTxInput {
    pub prev_out_hash: Vec<u8>,
    pub prev_out_index: u32,
    pub signature_script: Vec<u8>,
    pub sequence: u32,
}

impl From<&bitcoin::TxIn> for PrevTxInput {
    fn from(value: &bitcoin::TxIn) -> Self {
        PrevTxInput {
            prev_out_hash: (value.previous_output.txid.as_ref() as &[u8]).to_vec(),
            prev_out_index: value.previous_output.vout,
            signature_script: value.script_sig.as_bytes().to_vec(),
            sequence: value.sequence.to_consensus_u32(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct PrevTxOutput {
    pub value: u64,
    pub pubkey_script: Vec<u8>,
}

impl From<&bitcoin::TxOut> for PrevTxOutput {
    fn from(value: &bitcoin::TxOut) -> Self {
        PrevTxOutput {
            value: value.value.to_sat(),
            pubkey_script: value.script_pubkey.as_bytes().to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrevTx {
    pub version: u32,
    pub inputs: Vec<PrevTxInput>,
    pub outputs: Vec<PrevTxOutput>,
    pub locktime: u32,
}

impl From<&bitcoin::Transaction> for PrevTx {
    fn from(value: &bitcoin::Transaction) -> Self {
        PrevTx {
            version: value.version.0 as _,
            inputs: value.input.iter().map(PrevTxInput::from).collect(),
            outputs: value.output.iter().map(PrevTxOutput::from).collect(),
            locktime: value.lock_time.to_consensus_u32(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct TxInput {
    pub prev_out_hash: Vec<u8>,
    pub prev_out_index: u32,
    pub prev_out_value: u64,
    pub sequence: u32,
    pub keypath: Keypath,
    pub script_config_index: u32,
    // Can be None if all transaction inputs are Taproot.
    pub prev_tx: Option<PrevTx>,
}

impl TxInput {
    /// Returns the previous transaction when it is available.
    pub fn get_prev_tx(&self) -> Result<&PrevTx, Error> {
        self.prev_tx.as_ref().ok_or_else(|| {
            Error::BtcSign("input's previous transaction required but missing".into())
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct TxInternalOutput {
    pub keypath: Keypath,
    pub value: u64,
    pub script_config_index: u32,
}

#[derive(Debug, PartialEq)]
pub struct Payload {
    pub data: Vec<u8>,
    pub output_type: pb::BtcOutputType,
}

#[derive(thiserror::Error, Debug)]
pub enum PayloadError {
    #[error("unrecognized pubkey script")]
    Unrecognized,
    #[error("{0}")]
    InvalidOpReturn(&'static str),
}

impl Payload {
    pub fn from_pkscript(pkscript: &[u8]) -> Result<Payload, PayloadError> {
        let script = Script::from_bytes(pkscript);
        if script.is_p2pkh() {
            Ok(Payload {
                data: pkscript[3..23].to_vec(),
                output_type: pb::BtcOutputType::P2pkh,
            })
        } else if script.is_p2sh() {
            Ok(Payload {
                data: pkscript[2..22].to_vec(),
                output_type: pb::BtcOutputType::P2sh,
            })
        } else if script.is_p2wpkh() {
            Ok(Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2wpkh,
            })
        } else if script.is_p2wsh() {
            Ok(Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2wsh,
            })
        } else if script.is_p2tr() {
            Ok(Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2tr,
            })
        } else if matches!(script.as_bytes().first(), Some(byte) if *byte == opcodes::all::OP_RETURN.to_u8())
        {
            let mut instructions = script.instructions_minimal();
            match instructions.next() {
                Some(Ok(Instruction::Op(op))) if op == opcodes::all::OP_RETURN => {}
                _ => return Err(PayloadError::Unrecognized),
            }

            let payload = match instructions.next() {
                None => {
                    return Err(PayloadError::InvalidOpReturn(
                        "naked OP_RETURN is not supported",
                    ))
                }
                Some(Ok(Instruction::Op(op))) if op == opcodes::all::OP_PUSHBYTES_0 => Vec::new(),
                Some(Ok(Instruction::PushBytes(push))) => push.as_bytes().to_vec(),
                Some(Ok(_)) => {
                    return Err(PayloadError::InvalidOpReturn(
                        "no data push found after OP_RETURN",
                    ))
                }
                Some(Err(_)) => {
                    return Err(PayloadError::InvalidOpReturn(
                        "failed to parse OP_RETURN payload",
                    ))
                }
            };

            match instructions.next() {
                None => Ok(Payload {
                    data: payload,
                    output_type: pb::BtcOutputType::OpReturn,
                }),
                Some(Ok(_)) => Err(PayloadError::InvalidOpReturn(
                    "only one data push supported after OP_RETURN",
                )),
                Some(Err(_)) => Err(PayloadError::InvalidOpReturn(
                    "failed to parse OP_RETURN payload",
                )),
            }
        } else {
            Err(PayloadError::Unrecognized)
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct TxExternalOutput {
    pub payload: Payload,
    pub value: u64,
}

impl TryFrom<&bitcoin::TxOut> for TxExternalOutput {
    type Error = PsbtError;
    fn try_from(value: &bitcoin::TxOut) -> Result<Self, Self::Error> {
        Ok(TxExternalOutput {
            payload: Payload::from_pkscript(value.script_pubkey.as_bytes())?,
            value: value.value.to_sat(),
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum TxOutput {
    Internal(TxInternalOutput),
    External(TxExternalOutput),
}

#[derive(Debug, PartialEq)]
pub struct Transaction {
    pub script_configs: Vec<pb::BtcScriptConfigWithKeypath>,
    pub version: u32,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub locktime: u32,
}
// See https://github.com/spesmilo/electrum/blob/84dc181b6e7bb20e88ef6b98fb8925c5f645a765/electrum/ecc.py#L521-L523
#[derive(Debug, PartialEq)]
pub struct SignMessageSignature {
    pub sig: Vec<u8>,
    pub recid: u8,
    pub electrum_sig65: Vec<u8>,
}

#[derive(thiserror::Error, Debug)]
pub enum PsbtError {
    #[error("{0}")]
    SignError(#[from] bitcoin::psbt::SignError),
    #[error("Taproot pubkeys must be unique across the internal key and all leaf scripts.")]
    KeyNotUnique,
    #[error("Could not find our key in an input.")]
    KeyNotFound,
    #[error("Unrecognized/unsupported output type.")]
    UnknownOutputType,
    #[error("Invalid OP_RETURN script: {0}")]
    InvalidOpReturn(&'static str),
    #[error("Account script configs must contain a BIP44 account keypath.")]
    InvalidAccountKeypath,
}

impl From<PayloadError> for PsbtError {
    fn from(value: PayloadError) -> Self {
        match value {
            PayloadError::Unrecognized => PsbtError::UnknownOutputType,
            PayloadError::InvalidOpReturn(message) => PsbtError::InvalidOpReturn(message),
        }
    }
}

/// The wallet's signing key and derivation path for a PSBT input or output.
#[derive(Debug)]
pub enum OurKey {
    Segwit(bitcoin::secp256k1::PublicKey, Keypath),
    TaprootInternal(Keypath),
    TaprootScript(
        bitcoin::secp256k1::XOnlyPublicKey,
        bitcoin::taproot::TapLeafHash,
        Keypath,
    ),
}

impl OurKey {
    fn keypath(&self) -> &Keypath {
        match self {
            OurKey::Segwit(_, kp)
            | OurKey::TaprootInternal(kp)
            | OurKey::TaprootScript(_, _, kp) => kp,
        }
    }
}

trait PsbtOutputInfo {
    fn get_bip32_derivation(
        &self,
    ) -> &std::collections::BTreeMap<bitcoin::secp256k1::PublicKey, bitcoin::bip32::KeySource>;

    fn get_tap_internal_key(&self) -> Option<&bitcoin::secp256k1::XOnlyPublicKey>;
    fn get_tap_key_origins(
        &self,
    ) -> &std::collections::BTreeMap<
        bitcoin::secp256k1::XOnlyPublicKey,
        (
            Vec<bitcoin::taproot::TapLeafHash>,
            bitcoin::bip32::KeySource,
        ),
    >;
}

impl PsbtOutputInfo for &bitcoin::psbt::Input {
    fn get_bip32_derivation(
        &self,
    ) -> &std::collections::BTreeMap<bitcoin::secp256k1::PublicKey, bitcoin::bip32::KeySource> {
        &self.bip32_derivation
    }

    fn get_tap_internal_key(&self) -> Option<&bitcoin::secp256k1::XOnlyPublicKey> {
        self.tap_internal_key.as_ref()
    }

    fn get_tap_key_origins(
        &self,
    ) -> &std::collections::BTreeMap<
        bitcoin::secp256k1::XOnlyPublicKey,
        (
            Vec<bitcoin::taproot::TapLeafHash>,
            bitcoin::bip32::KeySource,
        ),
    > {
        &self.tap_key_origins
    }
}

impl PsbtOutputInfo for &bitcoin::psbt::Output {
    fn get_bip32_derivation(
        &self,
    ) -> &std::collections::BTreeMap<bitcoin::secp256k1::PublicKey, bitcoin::bip32::KeySource> {
        &self.bip32_derivation
    }

    fn get_tap_internal_key(&self) -> Option<&bitcoin::secp256k1::XOnlyPublicKey> {
        self.tap_internal_key.as_ref()
    }

    fn get_tap_key_origins(
        &self,
    ) -> &std::collections::BTreeMap<
        bitcoin::secp256k1::XOnlyPublicKey,
        (
            Vec<bitcoin::taproot::TapLeafHash>,
            bitcoin::bip32::KeySource,
        ),
    > {
        &self.tap_key_origins
    }
}

fn find_our_key<T: PsbtOutputInfo>(
    our_root_fingerprint: &[u8],
    output_info: T,
) -> Result<OurKey, PsbtError> {
    for (xonly, (leaf_hashes, (fingerprint, derivation_path))) in
        output_info.get_tap_key_origins().iter()
    {
        if &fingerprint[..] == our_root_fingerprint {
            // TODO: check for fingerprint collision

            if let Some(tap_internal_key) = output_info.get_tap_internal_key() {
                if tap_internal_key == xonly {
                    if !leaf_hashes.is_empty() {
                        // TODO change err msg, we don't support the
                        // same key as internal key and also in a leaf
                        // script.
                        return Err(PsbtError::KeyNotUnique);
                    }
                    return Ok(OurKey::TaprootInternal(derivation_path.into()));
                }
            }
            if leaf_hashes.len() != 1 {
                // TODO change err msg, per BIP-388 all pubkeys are
                // unique, so it can't be in multiple leafs.
                return Err(PsbtError::KeyNotUnique);
            }
            return Ok(OurKey::TaprootScript(
                *xonly,
                leaf_hashes[0],
                derivation_path.into(),
            ));
        }
    }
    for (pubkey, (fingerprint, derivation_path)) in output_info.get_bip32_derivation().iter() {
        if &fingerprint[..] == our_root_fingerprint {
            // TODO: check for fingerprint collision
            return Ok(OurKey::Segwit(*pubkey, derivation_path.into()));
        }
    }
    Err(PsbtError::KeyNotFound)
}

fn script_config_from_utxo(
    output: &bitcoin::TxOut,
    keypath: &Keypath,
    redeem_script: Option<&bitcoin::ScriptBuf>,
) -> Result<pb::BtcScriptConfigWithKeypath, PsbtError> {
    use pb::btc_script_config::SimpleType;
    let script = &output.script_pubkey;
    let simple_type = if script.is_p2wpkh() {
        SimpleType::P2wpkh
    } else if script.is_p2sh() && redeem_script.is_some_and(|script| script.is_p2wpkh()) {
        SimpleType::P2wpkhP2sh
    } else if script.is_p2tr() {
        SimpleType::P2tr
    } else {
        return Err(PsbtError::UnknownOutputType);
    };
    Ok(pb::BtcScriptConfigWithKeypath {
        script_config: Some(make_script_config_simple(simple_type)),
        keypath: keypath.hardened_prefix().to_vec(),
    })
}

/// A transaction lowered for signing, with keys in input order and config indices in output order.
#[derive(Debug)]
pub struct PsbtTransaction {
    pub transaction: Transaction,
    pub our_keys: Vec<OurKey>,
    pub output_script_configs: Vec<pb::BtcScriptConfigWithKeypath>,
    pub output_script_config_indices: Vec<Option<u32>>,
}

fn validate_psbt_key_counts(
    psbt: &bitcoin::psbt::Psbt,
    input_keys: usize,
    output_keys: usize,
) -> Result<(), Error> {
    if psbt.inputs.len() != psbt.unsigned_tx.input.len() || input_keys != psbt.inputs.len() {
        return Err(Error::BtcSign(
            "input signing keys must match PSBT inputs".into(),
        ));
    }
    if psbt.outputs.len() != psbt.unsigned_tx.output.len() || output_keys != psbt.outputs.len() {
        return Err(Error::BtcSign(
            "output signing keys must match PSBT outputs".into(),
        ));
    }
    Ok(())
}

impl Transaction {
    /// Lowers a PSBT using the device fingerprint to look up owned keys.
    pub fn from_psbt(
        firmware_version: &semver::Version,
        our_root_fingerprint: &[u8],
        psbt: &bitcoin::psbt::Psbt,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
    ) -> Result<PsbtTransaction, Error> {
        validate_psbt_key_counts(psbt, psbt.inputs.len(), psbt.outputs.len())?;
        let input_keys = psbt
            .inputs
            .iter()
            .map(|input| find_our_key(our_root_fingerprint, input))
            .collect::<Result<Vec<_>, _>>()?;
        let output_keys = psbt
            .outputs
            .iter()
            .map(|output| find_our_key(our_root_fingerprint, output).ok())
            .collect();
        Self::from_psbt_with_keys(
            firmware_version,
            psbt,
            force_script_config,
            input_keys,
            output_keys,
        )
    }

    /// Lowers a PSBT given caller-resolved keys in PSBT input and output order.
    ///
    /// An output key of `None` marks an external output. This does not verify key ownership.
    pub fn from_psbt_with_keys(
        firmware_version: &semver::Version,
        psbt: &bitcoin::psbt::Psbt,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
        input_keys: Vec<OurKey>,
        output_keys: Vec<Option<OurKey>>,
    ) -> Result<PsbtTransaction, Error> {
        validate_psbt_key_counts(psbt, input_keys.len(), output_keys.len())?;

        let is_script_config_forced = force_script_config.is_some();
        let mut script_configs = force_script_config.into_iter().collect::<Vec<_>>();
        let mut output_script_configs = Vec::new();
        let mut inputs = Vec::with_capacity(input_keys.len());

        for ((tx_input, psbt_input), our_key) in psbt
            .unsigned_tx
            .input
            .iter()
            .zip(&psbt.inputs)
            .zip(&input_keys)
        {
            // Unlike Psbt::spend_utxo, check that the non-witness output exists.
            let utxo = psbt_input
                .witness_utxo
                .as_ref()
                .or_else(|| {
                    psbt_input
                        .non_witness_utxo
                        .as_ref()?
                        .output
                        .get(tx_input.previous_output.vout as usize)
                })
                .ok_or(PsbtError::SignError(SignError::MissingSpendUtxo))?;
            let script_config_index = if is_script_config_forced {
                0
            } else {
                find_or_add_script_config(
                    &mut script_configs,
                    script_config_from_utxo(
                        utxo,
                        our_key.keypath(),
                        psbt_input.redeem_script.as_ref(),
                    )?,
                )
            };

            inputs.push(TxInput {
                prev_out_hash: (tx_input.previous_output.txid.as_ref() as &[u8]).to_vec(),
                prev_out_index: tx_input.previous_output.vout,
                prev_out_value: utxo.value.to_sat(),
                sequence: tx_input.sequence.to_consensus_u32(),
                keypath: our_key.keypath().clone(),
                script_config_index: script_config_index as _,
                prev_tx: psbt_input.non_witness_utxo.as_ref().map(PrevTx::from),
            });
        }

        let mut outputs = Vec::with_capacity(output_keys.len());
        let mut output_script_config_indices = Vec::with_capacity(output_keys.len());
        for ((tx_output, psbt_output), output_key) in psbt
            .unsigned_tx
            .output
            .iter()
            .zip(&psbt.outputs)
            .zip(output_keys)
        {
            let Some(
                OurKey::Segwit(_, keypath)
                | OurKey::TaprootInternal(keypath)
                | OurKey::TaprootScript(_, _, keypath),
            ) = output_key
            else {
                outputs.push(TxOutput::External(tx_output.try_into()?));
                output_script_config_indices.push(None);
                continue;
            };
            let (script_config_index, output_script_config_index) = if is_script_config_forced {
                // The forced config is used for all owned inputs and outputs, but still check
                // that a forced simple config has an account keypath.
                is_same_account(&script_configs, &script_configs[0])?;
                (0, None)
            } else {
                let script_config = script_config_from_utxo(
                    tx_output,
                    &keypath,
                    psbt_output.redeem_script.as_ref(),
                )?;
                if is_same_account(&script_configs, &script_config)? {
                    (
                        find_or_add_script_config(&mut script_configs, script_config) as u32,
                        None,
                    )
                } else if firmware_version >= &semver::Version::new(9, 22, 0) {
                    let index =
                        find_or_add_script_config(&mut output_script_configs, script_config);
                    // The input config index is ignored when the output config index is present.
                    (0, Some(index as u32))
                } else {
                    // Older firmware displays device-owned outputs of other accounts as external.
                    outputs.push(TxOutput::External(tx_output.try_into()?));
                    output_script_config_indices.push(None);
                    continue;
                }
            };
            outputs.push(TxOutput::Internal(TxInternalOutput {
                keypath,
                value: tx_output.value.to_sat(),
                script_config_index,
            }));
            output_script_config_indices.push(output_script_config_index);
        }

        Ok(PsbtTransaction {
            transaction: Transaction {
                script_configs,
                version: psbt.unsigned_tx.version.0 as _,
                inputs,
                outputs,
                locktime: psbt.unsigned_tx.lock_time.to_consensus_u32(),
            },
            our_keys: input_keys,
            output_script_configs,
            output_script_config_indices,
        })
    }
}

/// Applies device signatures to their PSBT inputs in input order, preserving other signatures.
///
/// A count mismatch is rejected before mutation. If a later signature is malformed, earlier
/// signatures may already have been applied.
///
/// Callers must verify the transaction signatures and any required Anti-Klepto proofs.
pub fn apply_signatures(
    psbt: &mut bitcoin::psbt::Psbt,
    signatures: &[Vec<u8>],
    our_keys: &[OurKey],
) -> Result<(), Error> {
    let num_inputs = psbt.unsigned_tx.input.len();
    if psbt.inputs.len() != num_inputs
        || signatures.len() != num_inputs
        || our_keys.len() != num_inputs
    {
        return Err(Error::BtcSign(
            "signatures and signing keys must match PSBT inputs".into(),
        ));
    }

    for ((input, signature), key) in psbt.inputs.iter_mut().zip(signatures).zip(our_keys) {
        match key {
            OurKey::Segwit(pubkey, _) => {
                let signature = crate::secp256k1::parse_signature_compact(signature)
                    .map_err(|_| Error::InvalidSignature)?;
                input.partial_sigs.insert(
                    bitcoin::PublicKey::new(*pubkey),
                    bitcoin::ecdsa::Signature::sighash_all(signature),
                );
            }
            OurKey::TaprootInternal(_) => {
                input.tap_key_sig = Some(
                    bitcoin::taproot::Signature::from_slice(signature)
                        .map_err(|_| Error::InvalidSignature)?,
                );
            }
            OurKey::TaprootScript(xonly, leaf_hash, _) => {
                let signature = bitcoin::taproot::Signature::from_slice(signature)
                    .map_err(|_| Error::InvalidSignature)?;
                input
                    .tap_script_sigs
                    .insert((*xonly, *leaf_hash), signature);
            }
        }
    }
    Ok(())
}

/// Create a single-sig script config.
pub fn make_script_config_simple(
    simple_type: pb::btc_script_config::SimpleType,
) -> pb::BtcScriptConfig {
    pb::BtcScriptConfig {
        config: Some(pb::btc_script_config::Config::SimpleType(
            simple_type.into(),
        )),
    }
}

#[derive(Clone, PartialEq)]
pub struct KeyOriginInfo {
    pub root_fingerprint: Option<bitcoin::bip32::Fingerprint>,
    pub keypath: Option<Keypath>,
    pub xpub: bitcoin::bip32::Xpub,
}

/// Converts an extended public key to the device's protobuf representation.
pub fn convert_xpub(xpub: &bitcoin::bip32::Xpub) -> pb::XPub {
    pb::XPub {
        depth: vec![xpub.depth],
        parent_fingerprint: xpub.parent_fingerprint[..].to_vec(),
        child_num: xpub.child_number.into(),
        chain_code: xpub.chain_code[..].to_vec(),
        public_key: xpub.public_key.serialize().to_vec(),
    }
}

impl From<KeyOriginInfo> for pb::KeyOriginInfo {
    fn from(value: KeyOriginInfo) -> Self {
        pb::KeyOriginInfo {
            root_fingerprint: value
                .root_fingerprint
                .map_or(vec![], |fp| fp.as_bytes().to_vec()),
            keypath: value.keypath.map_or(vec![], |kp| kp.to_vec()),
            xpub: Some(convert_xpub(&value.xpub)),
        }
    }
}

/// Create a multi-sig script config.
pub fn make_script_config_multisig(
    threshold: u32,
    xpubs: &[bitcoin::bip32::Xpub],
    our_xpub_index: u32,
    script_type: pb::btc_script_config::multisig::ScriptType,
) -> pb::BtcScriptConfig {
    pb::BtcScriptConfig {
        config: Some(pb::btc_script_config::Config::Multisig(
            pb::btc_script_config::Multisig {
                threshold,
                xpubs: xpubs.iter().map(convert_xpub).collect(),
                our_xpub_index,
                script_type: script_type as _,
            },
        )),
    }
}

/// Create a wallet policy script config according to the wallet policies BIP:
/// <https://github.com/bitcoin/bips/pull/1389>
///
/// At least one of the keys must be ours, i.e. contain our root fingerprint and a keypath to one of
/// our xpubs.
pub fn make_script_config_policy(policy: &str, keys: &[KeyOriginInfo]) -> pb::BtcScriptConfig {
    pb::BtcScriptConfig {
        config: Some(pb::btc_script_config::Config::Policy(
            pb::btc_script_config::Policy {
                policy: policy.into(),
                keys: keys.iter().cloned().map(pb::KeyOriginInfo::from).collect(),
            },
        )),
    }
}

fn is_taproot_simple(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
    matches!(
        script_config.script_config.as_ref(),
        Some(pb::BtcScriptConfig {
            config: Some(pb::btc_script_config::Config::SimpleType(simple_type)),
        }) if *simple_type == pb::btc_script_config::SimpleType::P2tr as i32
    )
}

fn is_taproot_policy(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
    matches!(
        script_config.script_config.as_ref(),
        Some(pb::BtcScriptConfig {
            config: Some(pb::btc_script_config::Config::Policy(policy)),
        })  if policy.policy.as_str().starts_with("tr("),
    )
}

/// Returns whether the script config uses Schnorr signatures.
pub fn is_schnorr(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
    is_taproot_simple(script_config) | is_taproot_policy(script_config)
}

fn is_simple_script_config(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
    matches!(
        script_config.script_config.as_ref(),
        Some(pb::BtcScriptConfig {
            config: Some(pb::btc_script_config::Config::SimpleType(_)),
        })
    )
}

fn is_same_account(
    input_script_configs: &[pb::BtcScriptConfigWithKeypath],
    output_script_config: &pb::BtcScriptConfigWithKeypath,
) -> Result<bool, PsbtError> {
    for input_script_config in input_script_configs {
        if is_simple_script_config(input_script_config) {
            let input_account = input_script_config
                .keypath
                .get(2)
                .ok_or(PsbtError::InvalidAccountKeypath)?;
            let output_account = output_script_config
                .keypath
                .get(2)
                .ok_or(PsbtError::InvalidAccountKeypath)?;
            if input_account != output_account {
                return Ok(false);
            }
        } else if input_script_config != output_script_config {
            return Ok(false);
        }
    }
    Ok(true)
}

fn find_or_add_script_config(
    script_configs: &mut Vec<pb::BtcScriptConfigWithKeypath>,
    script_config: pb::BtcScriptConfigWithKeypath,
) -> usize {
    if let Some(index) = script_configs
        .iter()
        .position(|existing| existing == &script_config)
    {
        index
    } else {
        script_configs.push(script_config);
        script_configs.len() - 1
    }
}

fn btc_request(request: pb::btc_request::Request) -> Request {
    Request::Btc(pb::BtcRequest {
        request: Some(request),
    })
}

/// Builds a request for a single extended public key.
pub fn xpub_request(
    coin: pb::BtcCoin,
    keypath: &Keypath,
    xpub_type: pb::btc_pub_request::XPubType,
    display: bool,
) -> Request {
    Request::BtcPub(pb::BtcPubRequest {
        coin: coin as _,
        keypath: keypath.to_vec(),
        display,
        output: Some(pb::btc_pub_request::Output::XpubType(xpub_type as _)),
    })
}

/// Builds a request for a Bitcoin address.
pub fn address_request(
    coin: pb::BtcCoin,
    keypath: &Keypath,
    script_config: pb::BtcScriptConfig,
    display: bool,
) -> Request {
    Request::BtcPub(pb::BtcPubRequest {
        coin: coin as _,
        keypath: keypath.to_vec(),
        display,
        output: Some(pb::btc_pub_request::Output::ScriptConfig(script_config)),
    })
}

fn script_config_registration(
    coin: pb::BtcCoin,
    script_config: pb::BtcScriptConfig,
    keypath_account: Option<&Keypath>,
) -> pb::BtcScriptConfigRegistration {
    pb::BtcScriptConfigRegistration {
        coin: coin as _,
        script_config: Some(script_config),
        keypath: keypath_account.map_or_else(Vec::new, Keypath::to_vec),
    }
}

/// Builds a request to check whether a wallet configuration is registered.
pub fn is_script_config_registered_request(
    coin: pb::BtcCoin,
    script_config: pb::BtcScriptConfig,
    keypath_account: Option<&Keypath>,
) -> Request {
    btc_request(pb::btc_request::Request::IsScriptConfigRegistered(
        pb::BtcIsScriptConfigRegisteredRequest {
            registration: Some(script_config_registration(
                coin,
                script_config,
                keypath_account,
            )),
        },
    ))
}

/// Builds a wallet registration request. An absent name prompts on the device.
pub fn register_script_config_request(
    coin: pb::BtcCoin,
    script_config: pb::BtcScriptConfig,
    keypath_account: Option<&Keypath>,
    xpub_type: pb::btc_register_script_config_request::XPubType,
    name: Option<&str>,
) -> Request {
    btc_request(pb::btc_request::Request::RegisterScriptConfig(
        pb::BtcRegisterScriptConfigRequest {
            registration: Some(script_config_registration(
                coin,
                script_config,
                keypath_account,
            )),
            name: name.unwrap_or_default().into(),
            xpub_type: xpub_type as _,
        },
    ))
}

/// Extracts the body of a Bitcoin-specific response.
pub fn btc_response(response: Response) -> Result<pb::btc_response::Response, Error> {
    match response {
        Response::Btc(pb::BtcResponse {
            response: Some(response),
        }) => Ok(response),
        _ => Err(Error::UnexpectedResponse),
    }
}

/// Parses the result of a wallet registration check.
pub fn parse_is_registered(response: Response) -> Result<bool, Error> {
    match btc_response(response)? {
        pb::btc_response::Response::IsScriptConfigRegistered(response) => {
            Ok(response.is_registered)
        }
        _ => Err(Error::UnexpectedResponse),
    }
}

/// Parses the acknowledgement of a wallet registration.
pub fn parse_registered(response: Response) -> Result<(), Error> {
    match btc_response(response)? {
        pb::btc_response::Response::Success(_) => Ok(()),
        _ => Err(Error::UnexpectedResponse),
    }
}

/// Builds the first request of a transaction signing exchange.
pub fn sign_init_request(
    coin: pb::BtcCoin,
    tx: &Transaction,
    output_script_configs: &[pb::BtcScriptConfigWithKeypath],
    format_unit: pb::btc_sign_init_request::FormatUnit,
) -> Request {
    Request::BtcSignInit(pb::BtcSignInitRequest {
        coin: coin as _,
        script_configs: tx.script_configs.clone(),
        output_script_configs: output_script_configs.to_vec(),
        version: tx.version,
        num_inputs: tx.inputs.len() as _,
        num_outputs: tx.outputs.len() as _,
        locktime: tx.locktime,
        format_unit: format_unit as _,
        contains_silent_payment_outputs: false,
    })
}

/// Builds a transaction input request, with a host commitment when using Anti-Klepto.
pub fn sign_input_request(input: &TxInput, host_nonce_commitment: Option<[u8; 32]>) -> Request {
    Request::BtcSignInput(pb::BtcSignInputRequest {
        prev_out_hash: input.prev_out_hash.clone(),
        prev_out_index: input.prev_out_index,
        prev_out_value: input.prev_out_value,
        sequence: input.sequence,
        keypath: input.keypath.to_vec(),
        script_config_index: input.script_config_index,
        host_nonce_commitment: host_nonce_commitment.map(|commitment| {
            pb::AntiKleptoHostNonceCommitment {
                commitment: commitment.to_vec(),
            }
        }),
    })
}

/// Builds a transaction output request. The separate config index applies only to internal outputs.
pub fn sign_output_request(output: &TxOutput, output_script_config_index: Option<u32>) -> Request {
    Request::BtcSignOutput(match output {
        TxOutput::Internal(output) => pb::BtcSignOutputRequest {
            ours: true,
            value: output.value,
            keypath: output.keypath.to_vec(),
            script_config_index: output.script_config_index,
            output_script_config_index,
            ..Default::default()
        },
        TxOutput::External(output) => pb::BtcSignOutputRequest {
            ours: false,
            value: output.value,
            r#type: output.payload.output_type as _,
            payload: output.payload.data.clone(),
            ..Default::default()
        },
    })
}

/// Builds a request describing a previous transaction.
pub fn prevtx_init_request(tx: &PrevTx) -> Request {
    btc_request(pb::btc_request::Request::PrevtxInit(
        pb::BtcPrevTxInitRequest {
            version: tx.version,
            num_inputs: tx.inputs.len() as _,
            num_outputs: tx.outputs.len() as _,
            locktime: tx.locktime,
        },
    ))
}

/// Builds a request carrying a previous transaction's input.
pub fn prevtx_input_request(input: &PrevTxInput) -> Request {
    btc_request(pb::btc_request::Request::PrevtxInput(
        pb::BtcPrevTxInputRequest {
            prev_out_hash: input.prev_out_hash.clone(),
            prev_out_index: input.prev_out_index,
            signature_script: input.signature_script.clone(),
            sequence: input.sequence,
        },
    ))
}

/// Builds a request carrying a previous transaction's output.
pub fn prevtx_output_request(output: &PrevTxOutput) -> Request {
    btc_request(pb::btc_request::Request::PrevtxOutput(
        pb::BtcPrevTxOutputRequest {
            value: output.value,
            pubkey_script: output.pubkey_script.clone(),
        },
    ))
}

/// Reveals the host nonce to request an Anti-Klepto signature.
pub fn antiklepto_signature_request(host_nonce: &[u8; 32]) -> Request {
    btc_request(pb::btc_request::Request::AntikleptoSignature(
        pb::AntiKleptoSignatureRequest {
            host_nonce: host_nonce.to_vec(),
        },
    ))
}

/// Starts a message signing exchange with an Anti-Klepto host commitment.
pub fn sign_message_request(
    coin: pb::BtcCoin,
    script_config: pb::BtcScriptConfigWithKeypath,
    message: &[u8],
    host_nonce_commitment: [u8; 32],
) -> Request {
    btc_request(pb::btc_request::Request::SignMessage(
        pb::BtcSignMessageRequest {
            coin: coin as _,
            script_config: Some(script_config),
            msg: message.to_vec(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: host_nonce_commitment.to_vec(),
            }),
        },
    ))
}

/// Parses either a top-level or BTC-nested signing reply without interpreting its phase.
pub fn parse_sign_next(response: Response) -> Result<pb::BtcSignNextResponse, Error> {
    match response {
        Response::BtcSignNext(next) => Ok(next),
        Response::Btc(pb::BtcResponse {
            response: Some(pb::btc_response::Response::SignNext(next)),
        }) => Ok(next),
        _ => Err(Error::UnexpectedResponse),
    }
}

#[cfg(feature = "client")]
fn parse_client_sign_next(
    response: Response,
    nested: bool,
) -> Result<pb::BtcSignNextResponse, Error> {
    if nested != matches!(response, Response::Btc(_)) {
        return Err(Error::UnexpectedResponse);
    }
    parse_sign_next(response)
}

/// Extracts the device's Anti-Klepto commitment in a message signing exchange.
pub fn parse_signer_commitment(response: Response) -> Result<Vec<u8>, Error> {
    match btc_response(response)? {
        pb::btc_response::Response::AntikleptoSignerCommitment(response) => Ok(response.commitment),
        _ => Err(Error::UnexpectedResponse),
    }
}

fn message_signature_bytes(response: Response) -> Result<Vec<u8>, Error> {
    match btc_response(response)? {
        pb::btc_response::Response::SignMessage(response) => Ok(response.signature),
        _ => Err(Error::UnexpectedResponse),
    }
}

fn format_validated_message_signature(mut signature: Vec<u8>) -> SignMessageSignature {
    let recid = signature[64];
    signature.truncate(64);
    let mut electrum_sig65 = Vec::with_capacity(65);
    // Electrum uses 27 plus the recovery ID, plus 4 for compressed pubkeys.
    electrum_sig65.push(27 + 4 + recid);
    electrum_sig65.extend_from_slice(&signature);
    SignMessageSignature {
        sig: signature,
        recid,
        electrum_sig65,
    }
}

/// Parses and validates a recoverable message signature's encoding.
///
/// This does not verify the Anti-Klepto nonce contribution.
pub fn parse_message_signature(response: Response) -> Result<SignMessageSignature, Error> {
    let signature = message_signature_bytes(response)?;
    crate::secp256k1::validate_signature_recoverable(&signature)
        .map_err(crate::antiklepto::Error::from)?;
    Ok(format_validated_message_signature(signature))
}

/// Checks that the firmware matches the specified semver requirement.
pub fn validate_version(
    firmware_version: &semver::Version,
    comparison: &'static str,
) -> Result<(), Error> {
    if semver::VersionReq::parse(comparison)
        .map_err(|_| Error::Unknown)?
        .matches(firmware_version)
    {
        Ok(())
    } else {
        Err(Error::Version(comparison))
    }
}

/// Checks output index cardinality and the firmware requirements for raw transaction signing.
///
/// An empty `output_script_config_indices` slice means no output has a separate config index.
pub fn validate_signing(
    firmware_version: &semver::Version,
    transaction: &Transaction,
    output_script_configs: &[pb::BtcScriptConfigWithKeypath],
    output_script_config_indices: &[Option<u32>],
) -> Result<(), Error> {
    if !output_script_config_indices.is_empty()
        && output_script_config_indices.len() != transaction.outputs.len()
    {
        return Err(Error::BtcSign(
            "output script config indices must match transaction outputs".into(),
        ));
    }
    validate_version(firmware_version, ">=9.4.0")?; // Anti-Klepto
    if transaction
        .script_configs
        .iter()
        .chain(output_script_configs)
        .any(is_taproot_simple)
    {
        validate_version(firmware_version, ">=9.10.0")?;
    }
    if !output_script_configs.is_empty() || output_script_config_indices.iter().any(Option::is_some)
    {
        validate_version(firmware_version, ">=9.22.0")?;
    }
    if transaction.outputs.iter().any(|output| {
        matches!(
            output,
            TxOutput::External(tx_output)
                if tx_output.payload.output_type == pb::BtcOutputType::OpReturn
        )
    }) {
        validate_version(firmware_version, ">=9.24.0")?;
    }
    Ok(())
}

#[cfg(feature = "client")]
impl<R: Runtime> PairedBitBox<R> {
    /// Retrieves an xpub. For non-standard keypaths, a warning is displayed on the BitBox even if
    /// `display` is false.
    pub async fn btc_xpub(
        &self,
        coin: pb::BtcCoin,
        keypath: &Keypath,
        xpub_type: pb::btc_pub_request::XPubType,
        display: bool,
    ) -> Result<String, Error> {
        let _api_call = self.begin_api_call().await;
        self.btc_xpub_inner(coin, keypath, xpub_type, display).await
    }

    async fn btc_xpub_inner(
        &self,
        coin: pb::BtcCoin,
        keypath: &Keypath,
        xpub_type: pb::btc_pub_request::XPubType,
        display: bool,
    ) -> Result<String, Error> {
        crate::api::parse_pub(
            self.query_proto(xpub_request(coin, keypath, xpub_type, display))
                .await?,
        )
    }

    /// Retrieves multiple xpubs at once. Only standard keypaths are allowed.
    /// On firmware <v9.24.0, this falls back to fetching each xpub one-by-one.
    pub async fn btc_xpubs(
        &self,
        coin: pb::BtcCoin,
        keypaths: &[Keypath],
        xpub_type: pb::btc_xpubs_request::XPubType,
    ) -> Result<Vec<String>, Error> {
        let _api_call = self.begin_api_call().await;
        if self.validate_version(">=9.24.0").is_err() {
            // Fallback to fetching them one-by-one on older firmware.
            let mut xpubs = Vec::<String>::with_capacity(keypaths.len());
            for keypath in keypaths {
                let converted_xpub_type = match xpub_type {
                    pb::btc_xpubs_request::XPubType::Unknown => return Err(Error::Unknown),
                    pb::btc_xpubs_request::XPubType::Tpub => pb::btc_pub_request::XPubType::Tpub,
                    pb::btc_xpubs_request::XPubType::Xpub => pb::btc_pub_request::XPubType::Xpub,
                };
                let xpub = self
                    .btc_xpub_inner(coin, keypath, converted_xpub_type, false)
                    .await?;
                xpubs.push(xpub);
            }
            return Ok(xpubs);
        }
        match self
            .query_proto_btc(pb::btc_request::Request::Xpubs(pb::BtcXpubsRequest {
                coin: coin as _,
                xpub_type: xpub_type as _,
                keypaths: keypaths.iter().map(|kp| kp.into()).collect(),
            }))
            .await?
        {
            pb::btc_response::Response::Pubs(pb::PubsResponse { pubs }) => Ok(pubs),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Retrieves a Bitcoin address at the provided keypath.
    ///
    /// For the simple script configs (single-sig), the keypath must follow the
    /// BIP44/BIP49/BIP84/BIP86 conventions.
    pub async fn btc_address(
        &self,
        coin: pb::BtcCoin,
        keypath: &Keypath,
        script_config: &pb::BtcScriptConfig,
        display: bool,
    ) -> Result<String, Error> {
        let _api_call = self.begin_api_call().await;
        crate::api::parse_pub(
            self.query_proto(address_request(
                coin,
                keypath,
                script_config.clone(),
                display,
            ))
            .await?,
        )
    }

    async fn query_proto_btc(
        &self,
        request: pb::btc_request::Request,
    ) -> Result<pb::btc_response::Response, Error> {
        btc_response(self.query_proto(btc_request(request)).await?)
    }

    async fn get_next_response(&self, request: Request) -> Result<pb::BtcSignNextResponse, Error> {
        let nested = matches!(request, Request::Btc(_));
        parse_client_sign_next(self.query_proto(request).await?, nested)
    }

    /// Sign a Bitcoin transaction. Returns one 64 byte signature (compact serlization of the R and
    /// S values) per input.
    pub async fn btc_sign(
        &self,
        coin: pb::BtcCoin,
        transaction: &Transaction,
        format_unit: pb::btc_sign_init_request::FormatUnit,
    ) -> Result<Vec<Vec<u8>>, Error> {
        let _api_call = self.begin_api_call().await;
        self.btc_sign_inner(coin, transaction, &[], &[], format_unit)
            .await
    }

    async fn btc_sign_inner(
        &self,
        coin: pb::BtcCoin,
        transaction: &Transaction,
        output_script_configs: &[pb::BtcScriptConfigWithKeypath],
        output_script_config_indices: &[Option<u32>],
        format_unit: pb::btc_sign_init_request::FormatUnit,
    ) -> Result<Vec<Vec<u8>>, Error> {
        validate_signing(
            self.version(),
            transaction,
            output_script_configs,
            output_script_config_indices,
        )?;

        let mut sigs = Vec::with_capacity(transaction.inputs.len());

        let mut next_response = self
            .get_next_response(sign_init_request(
                coin,
                transaction,
                output_script_configs,
                format_unit,
            ))
            .await?;

        let mut is_inputs_pass2 = false;
        loop {
            match pb::btc_sign_next_response::Type::try_from(next_response.r#type)
                .map_err(|_| Error::UnexpectedResponse)?
            {
                pb::btc_sign_next_response::Type::Input => {
                    let input_index: usize = next_response.index as _;
                    let tx_input: &TxInput = &transaction.inputs[input_index];

                    let input_is_schnorr = is_schnorr(
                        &transaction.script_configs[tx_input.script_config_index as usize],
                    );
                    let perform_antiklepto = is_inputs_pass2 && !input_is_schnorr;
                    let host_nonce = if perform_antiklepto {
                        Some(crate::antiklepto::gen_host_nonce()?)
                    } else {
                        None
                    };
                    next_response = self
                        .get_next_response(sign_input_request(
                            tx_input,
                            host_nonce
                                .as_ref()
                                .map(|nonce| crate::antiklepto::host_commit(nonce)),
                        ))
                        .await?;

                    if let Some(host_nonce) = host_nonce {
                        if next_response.r#type
                            != pb::btc_sign_next_response::Type::HostNonce as i32
                        {
                            return Err(Error::UnexpectedResponse);
                        }
                        if let Some(pb::AntiKleptoSignerCommitment { commitment }) =
                            next_response.anti_klepto_signer_commitment
                        {
                            next_response = self
                                .get_next_response(antiklepto_signature_request(&host_nonce))
                                .await?;
                            if !next_response.has_signature {
                                return Err(Error::UnexpectedResponse);
                            }
                            crate::antiklepto::verify_ecdsa(
                                &host_nonce,
                                &commitment,
                                &next_response.signature,
                            )?
                        } else {
                            return Err(Error::UnexpectedResponse);
                        }
                    }

                    if is_inputs_pass2 {
                        if !next_response.has_signature {
                            return Err(Error::UnexpectedResponse);
                        }
                        sigs.push(std::mem::take(&mut next_response.signature));
                    }
                    if input_index == transaction.inputs.len() - 1 {
                        is_inputs_pass2 = true
                    }
                }
                pb::btc_sign_next_response::Type::PrevtxInit => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    next_response = self.get_next_response(prevtx_init_request(prevtx)).await?;
                }
                pb::btc_sign_next_response::Type::PrevtxInput => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    let prevtx_input: &PrevTxInput =
                        &prevtx.inputs[next_response.prev_index as usize];
                    next_response = self
                        .get_next_response(prevtx_input_request(prevtx_input))
                        .await?;
                }
                pb::btc_sign_next_response::Type::PrevtxOutput => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    let prevtx_output: &PrevTxOutput =
                        &prevtx.outputs[next_response.prev_index as usize];
                    next_response = self
                        .get_next_response(prevtx_output_request(prevtx_output))
                        .await?;
                }
                pb::btc_sign_next_response::Type::Output => {
                    let output_index = next_response.index as usize;
                    let tx_output: &TxOutput = &transaction.outputs[output_index];
                    let output_script_config_index = output_script_config_indices
                        .get(output_index)
                        .copied()
                        .flatten();
                    next_response = self
                        .get_next_response(sign_output_request(
                            tx_output,
                            output_script_config_index,
                        ))
                        .await?;
                }
                pb::btc_sign_next_response::Type::Done => break,
                pb::btc_sign_next_response::Type::HostNonce => {
                    return Err(Error::UnexpectedResponse);
                }
                _ => return Err(Error::UnexpectedResponse),
            }
        }
        Ok(sigs)
    }

    /// Sign a PSBT.
    ///
    /// If `force_script_config` is None, we attempt to infer the involved script configs. For the
    /// simple script config (single sig), we infer the script config from the involved redeem
    /// scripts and provided derviation paths.
    ///
    /// Multisig and policy configs are currently not inferred and must be provided using
    /// `force_script_config`.
    pub async fn btc_sign_psbt(
        &self,
        coin: pb::BtcCoin,
        psbt: &mut bitcoin::psbt::Psbt,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
        format_unit: pb::btc_sign_init_request::FormatUnit,
    ) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        // since v9.15.0, the BitBox02 accepts "internal" outputs (ones sent to the BitBox02 with
        // the keypath) even if the keypath is not a change keypath. PSBTs often contain the key
        // origin info in outputs even in regular send-to-self outputs.
        self.validate_version(">=9.15.0")?;

        let our_root_fingerprint = self.root_fingerprint_inner().await?;
        let psbt_transaction = Transaction::from_psbt(
            self.version(),
            &our_root_fingerprint,
            psbt,
            force_script_config,
        )?;
        let signatures = self
            .btc_sign_inner(
                coin,
                &psbt_transaction.transaction,
                &psbt_transaction.output_script_configs,
                &psbt_transaction.output_script_config_indices,
                format_unit,
            )
            .await?;
        apply_signatures(psbt, &signatures, &psbt_transaction.our_keys)
    }

    /// Sign a message.
    pub async fn btc_sign_message(
        &self,
        coin: pb::BtcCoin,
        script_config: pb::BtcScriptConfigWithKeypath,
        msg: &[u8],
    ) -> Result<SignMessageSignature, Error> {
        let _api_call = self.begin_api_call().await;
        self.validate_version(">=9.5.0")?;

        let host_nonce = crate::antiklepto::gen_host_nonce()?;
        let signer_commitment = parse_signer_commitment(
            self.query_proto(sign_message_request(
                coin,
                script_config,
                msg,
                crate::antiklepto::host_commit(&host_nonce),
            ))
            .await?,
        )?;
        let signature = message_signature_bytes(
            self.query_proto(antiklepto_signature_request(&host_nonce))
                .await?,
        )?;
        // The nonce verifier also validates the encoding before formatting.
        crate::antiklepto::verify_recoverable_ecdsa(&host_nonce, &signer_commitment, &signature)?;
        Ok(format_validated_message_signature(signature))
    }

    /// Before a multisig or policy script config can be used to display receive addresses or sign
    /// transactions, it must be registered on the device. This function checks if the script config
    /// was already registered.
    ///
    /// `keypath_account` must be set if the script config is multisig, and can be `None` if it is a
    /// policy.
    pub async fn btc_is_script_config_registered(
        &self,
        coin: pb::BtcCoin,
        script_config: &pb::BtcScriptConfig,
        keypath_account: Option<&Keypath>,
    ) -> Result<bool, Error> {
        let _api_call = self.begin_api_call().await;
        parse_is_registered(
            self.query_proto(is_script_config_registered_request(
                coin,
                script_config.clone(),
                keypath_account,
            ))
            .await?,
        )
    }

    /// Before a multisig or policy script config can be used to display receive addresses or sign
    /// transcations, it must be registered on the device.
    ///
    /// If no name is provided, the user will be asked to enter it on the device instead.  If
    /// provided, it must be non-empty, smaller or equal to 30 chars, consist only of printable
    /// ASCII characters, and contain no whitespace other than spaces.
    ///
    ///
    /// `keypath_account` must be set if the script config is multisig, and can be `None` if it is a
    /// policy.
    pub async fn btc_register_script_config(
        &self,
        coin: pb::BtcCoin,
        script_config: &pb::BtcScriptConfig,
        keypath_account: Option<&Keypath>,
        xpub_type: pb::btc_register_script_config_request::XPubType,
        name: Option<&str>,
    ) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        parse_registered(
            self.query_proto(register_script_config_request(
                coin,
                script_config.clone(),
                keypath_account,
                xpub_type,
                name,
            ))
            .await?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_response(response: pb::btc_response::Response) -> Response {
        Response::Btc(pb::BtcResponse {
            response: Some(response),
        })
    }

    #[test]
    fn one_shot_requests_preserve_variants_and_optional_fields() {
        use pb::btc_pub_request::{Output, XPubType};
        use pb::btc_register_script_config_request::XPubType as RegistrationXpub;
        use pb::btc_request::Request as BtcRequest;

        let coin = pb::BtcCoin::Tbtc;
        let keypath = Keypath::try_from("m/1").unwrap();
        let config = make_script_config_simple(pb::btc_script_config::SimpleType::P2wpkh);
        // The zero-valued Tpub must still be present in the oneof.
        assert_eq!(
            xpub_request(coin, &keypath, XPubType::Tpub, false),
            Request::BtcPub(pb::BtcPubRequest {
                coin: coin as _,
                keypath: vec![1],
                display: false,
                output: Some(Output::XpubType(0)),
            }),
        );
        assert_eq!(
            address_request(coin, &keypath, config.clone(), true),
            Request::BtcPub(pb::BtcPubRequest {
                coin: coin as _,
                keypath: vec![1],
                display: true,
                output: Some(Output::ScriptConfig(config.clone())),
            }),
        );
        assert_eq!(
            is_script_config_registered_request(coin, config.clone(), None),
            Request::Btc(pb::BtcRequest {
                request: Some(BtcRequest::IsScriptConfigRegistered(
                    pb::BtcIsScriptConfigRegisteredRequest {
                        registration: Some(pb::BtcScriptConfigRegistration {
                            coin: coin as _,
                            keypath: vec![],
                            script_config: Some(config.clone()),
                        }),
                    },
                )),
            }),
        );
        for (name, expected) in [(None, ""), (Some("wallet"), "wallet")] {
            assert_eq!(
                register_script_config_request(
                    coin,
                    config.clone(),
                    Some(&keypath),
                    RegistrationXpub::AutoXpubTpub,
                    name,
                ),
                Request::Btc(pb::BtcRequest {
                    request: Some(BtcRequest::RegisterScriptConfig(
                        pb::BtcRegisterScriptConfigRequest {
                            registration: Some(pb::BtcScriptConfigRegistration {
                                coin: coin as _,
                                keypath: vec![1],
                                script_config: Some(config.clone()),
                            }),
                            name: expected.into(),
                            xpub_type: RegistrationXpub::AutoXpubTpub as _,
                        },
                    )),
                }),
            );
        }
    }

    #[test]
    fn registration_parsers_require_btc_reply_variants() {
        use pb::btc_response::Response as BtcResponse;

        for is_registered in [false, true] {
            let response = nested_response(BtcResponse::IsScriptConfigRegistered(
                pb::BtcIsScriptConfigRegisteredResponse { is_registered },
            ));
            assert_eq!(
                parse_is_registered(response.clone()).unwrap(),
                is_registered
            );
            assert!(matches!(
                parse_registered(response),
                Err(Error::UnexpectedResponse)
            ));
        }
        let response = nested_response(BtcResponse::Success(pb::BtcSuccess {}));
        assert!(parse_registered(response.clone()).is_ok());
        assert!(matches!(
            parse_is_registered(response),
            Err(Error::UnexpectedResponse)
        ));
        assert!(matches!(
            btc_response(Response::Btc(pb::BtcResponse { response: None })),
            Err(Error::UnexpectedResponse)
        ));
        assert!(matches!(
            parse_registered(Response::Success(pb::Success {})),
            Err(Error::UnexpectedResponse)
        ));
    }

    #[test]
    fn signing_requests_preserve_payloads_and_presence() {
        use pb::btc_request::Request as BtcRequest;
        use pb::btc_sign_init_request::FormatUnit;

        let coin = pb::BtcCoin::Tbtc;
        let config = pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2wpkh,
            )),
            keypath: vec![1, 2, 3],
        };
        let keypath = Keypath::try_from("m/7").unwrap();
        let prevtx = PrevTx {
            version: 1,
            locktime: 9,
            inputs: vec![PrevTxInput {
                prev_out_hash: vec![1; 32],
                prev_out_index: 2,
                sequence: 3,
                signature_script: vec![4],
            }],
            outputs: vec![PrevTxOutput {
                value: 500,
                pubkey_script: vec![6],
            }],
        };
        let tx = Transaction {
            version: 2,
            locktime: 42,
            script_configs: vec![config.clone()],
            inputs: vec![TxInput {
                prev_out_hash: vec![7; 32],
                prev_out_index: 8,
                prev_out_value: 900,
                sequence: 10,
                keypath: keypath.clone(),
                script_config_index: 11,
                prev_tx: Some(prevtx),
            }],
            outputs: vec![
                TxOutput::Internal(TxInternalOutput {
                    keypath,
                    value: 100,
                    script_config_index: 12,
                }),
                TxOutput::External(TxExternalOutput {
                    payload: Payload {
                        output_type: pb::BtcOutputType::P2wpkh,
                        data: vec![13; 20],
                    },
                    value: 200,
                }),
            ],
        };
        let output_config = pb::BtcScriptConfigWithKeypath {
            keypath: vec![1, 2, 4],
            ..config.clone()
        };
        assert_eq!(
            sign_init_request(
                coin,
                &tx,
                std::slice::from_ref(&output_config),
                FormatUnit::Sat
            ),
            Request::BtcSignInit(pb::BtcSignInitRequest {
                coin: coin as _,
                script_configs: vec![config.clone()],
                output_script_configs: vec![output_config],
                version: 2,
                num_inputs: 1,
                num_outputs: 2,
                locktime: 42,
                format_unit: FormatUnit::Sat as _,
                contains_silent_payment_outputs: false,
            }),
        );
        let commitment = [14; 32];
        for host_commitment in [None, Some(commitment)] {
            assert_eq!(
                sign_input_request(&tx.inputs[0], host_commitment),
                Request::BtcSignInput(pb::BtcSignInputRequest {
                    prev_out_hash: vec![7; 32],
                    prev_out_index: 8,
                    prev_out_value: 900,
                    sequence: 10,
                    keypath: vec![7],
                    script_config_index: 11,
                    host_nonce_commitment: host_commitment.map(|_| {
                        pb::AntiKleptoHostNonceCommitment {
                            commitment: vec![14; 32],
                        }
                    }),
                }),
            );
        }
        for index in [None, Some(0)] {
            assert_eq!(
                sign_output_request(&tx.outputs[0], index),
                Request::BtcSignOutput(pb::BtcSignOutputRequest {
                    ours: true,
                    value: 100,
                    keypath: vec![7],
                    script_config_index: 12,
                    output_script_config_index: index,
                    ..Default::default()
                }),
            );
        }
        assert_eq!(
            sign_output_request(&tx.outputs[1], Some(0)),
            Request::BtcSignOutput(pb::BtcSignOutputRequest {
                r#type: pb::BtcOutputType::P2wpkh as _,
                payload: vec![13; 20],
                value: 200,
                ..Default::default()
            }),
        );

        let nested = |request| {
            Request::Btc(pb::BtcRequest {
                request: Some(request),
            })
        };
        let prevtx = tx.inputs[0].get_prev_tx().unwrap();
        assert_eq!(
            prevtx_init_request(prevtx),
            nested(BtcRequest::PrevtxInit(pb::BtcPrevTxInitRequest {
                version: 1,
                num_inputs: 1,
                num_outputs: 1,
                locktime: 9,
            })),
        );
        assert_eq!(
            prevtx_input_request(&prevtx.inputs[0]),
            nested(BtcRequest::PrevtxInput(pb::BtcPrevTxInputRequest {
                prev_out_hash: vec![1; 32],
                prev_out_index: 2,
                sequence: 3,
                signature_script: vec![4],
            })),
        );
        assert_eq!(
            prevtx_output_request(&prevtx.outputs[0]),
            nested(BtcRequest::PrevtxOutput(pb::BtcPrevTxOutputRequest {
                value: 500,
                pubkey_script: vec![6],
            })),
        );
        assert_eq!(
            sign_message_request(coin, config.clone(), b"test", commitment),
            nested(BtcRequest::SignMessage(pb::BtcSignMessageRequest {
                coin: coin as _,
                script_config: Some(config),
                msg: b"test".to_vec(),
                host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                    commitment: vec![14; 32],
                }),
            })),
        );
        assert_eq!(
            antiklepto_signature_request(&[15; 32]),
            nested(BtcRequest::AntikleptoSignature(
                pb::AntiKleptoSignatureRequest {
                    host_nonce: vec![15; 32],
                }
            )),
        );
    }

    #[test]
    fn sign_next_accepts_both_response_nestings() {
        use pb::btc_response::Response as BtcResponse;
        let next = pb::BtcSignNextResponse {
            r#type: pb::btc_sign_next_response::Type::PrevtxOutput as _,
            index: 2,
            prev_index: 3,
            ..Default::default()
        };
        assert_eq!(
            parse_sign_next(Response::BtcSignNext(next.clone())).unwrap(),
            next
        );
        assert_eq!(
            parse_sign_next(nested_response(BtcResponse::SignNext(next.clone()))).unwrap(),
            next,
        );
        assert!(matches!(
            parse_sign_next(nested_response(BtcResponse::Success(pb::BtcSuccess {}))),
            Err(Error::UnexpectedResponse)
        ));
    }

    #[cfg(feature = "client")]
    #[test]
    fn client_sign_next_rejects_the_wrong_response_envelope() {
        let next = pb::BtcSignNextResponse::default();
        for (response, nested) in [
            (Response::BtcSignNext(next.clone()), false),
            (
                nested_response(pb::btc_response::Response::SignNext(next.clone())),
                true,
            ),
        ] {
            assert!(matches!(
                parse_client_sign_next(response.clone(), !nested),
                Err(Error::UnexpectedResponse)
            ));
            assert_eq!(parse_client_sign_next(response, nested).unwrap(), next);
        }
    }

    #[test]
    fn message_reply_parsing_validates_encoding_but_not_nonce() {
        use pb::btc_response::Response as BtcResponse;

        let commitment = nested_response(BtcResponse::AntikleptoSignerCommitment(
            pb::AntiKleptoSignerCommitment {
                commitment: vec![42; 33],
            },
        ));
        assert_eq!(
            parse_signer_commitment(commitment.clone()).unwrap(),
            vec![42; 33]
        );
        assert!(matches!(
            parse_message_signature(commitment),
            Err(Error::UnexpectedResponse)
        ));
        let message = |signature| {
            nested_response(BtcResponse::SignMessage(pb::BtcSignMessageResponse {
                signature,
            }))
        };
        // Compact r = 1, s = 1: valid scalars irrespective of any message or nonce.
        let mut compact = vec![0; 64];
        compact[31] = 1;
        compact[63] = 1;
        assert!(matches!(
            parse_signer_commitment(message(compact.clone())),
            Err(Error::UnexpectedResponse)
        ));
        for recid in [0, 3] {
            let mut recoverable = compact.clone();
            recoverable.push(recid);
            let parsed = parse_message_signature(message(recoverable)).unwrap();
            assert_eq!(parsed.sig, compact);
            assert_eq!(parsed.recid, recid);
            assert_eq!(parsed.electrum_sig65[0], 31 + recid);
            assert_eq!(&parsed.electrum_sig65[1..], compact.as_slice());
        }
        let mut bad_recid = compact.clone();
        bad_recid.push(4);
        for malformed in [compact, bad_recid, vec![0; 65]] {
            assert!(matches!(
                parse_message_signature(message(malformed)),
                Err(Error::AntiKlepto(
                    crate::antiklepto::Error::InvalidSignature(_)
                ))
            ));
        }
    }

    #[test]
    fn raw_signing_checks_versions_and_output_index_count() {
        use pb::btc_script_config::SimpleType;
        use semver::Version;

        let mut tx = Transaction {
            version: 2,
            locktime: 0,
            inputs: vec![],
            script_configs: vec![],
            outputs: vec![],
        };
        let old = Version::new(9, 3, 0);
        assert!(matches!(
            validate_version(&old, "not semver"),
            Err(Error::Unknown)
        ));
        assert!(matches!(
            validate_signing(&old, &tx, &[], &[]),
            Err(Error::Version(">=9.4.0"))
        ));
        assert!(validate_signing(&Version::new(9, 4, 0), &tx, &[], &[]).is_ok());

        let taproot = pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(SimpleType::P2tr)),
            keypath: vec![],
        };
        assert!(is_schnorr(&taproot));
        assert!(matches!(
            validate_signing(
                &Version::new(9, 9, 0),
                &tx,
                std::slice::from_ref(&taproot),
                &[]
            ),
            Err(Error::Version(">=9.10.0"))
        ));
        tx.script_configs.push(taproot);
        assert!(matches!(
            validate_signing(&Version::new(9, 9, 0), &tx, &[], &[]),
            Err(Error::Version(">=9.10.0"))
        ));
        assert!(validate_signing(&Version::new(9, 10, 0), &tx, &[], &[]).is_ok());

        tx.outputs.push(TxOutput::External(TxExternalOutput {
            value: 0,
            payload: Payload::from_pkscript(&[0x6a, 1, 42]).unwrap(),
        }));
        assert!(matches!(
            validate_signing(&Version::new(9, 23, 0), &tx, &[], &[]),
            Err(Error::Version(">=9.24.0"))
        ));
        let op_return_version = Version::new(9, 24, 0);
        assert!(validate_signing(&op_return_version, &tx, &[], &[None]).is_ok());
        assert!(matches!(
            validate_signing(&op_return_version, &tx, &[], &[None, None]),
            Err(Error::BtcSign(message))
                if message == "output script config indices must match transaction outputs"
        ));
    }

    fn psbt_fixture() -> (
        bitcoin::psbt::Psbt,
        Fingerprint,
        bitcoin::secp256k1::PublicKey,
    ) {
        use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
        use bitcoin::{absolute, psbt::Psbt, transaction, Amount, CompressedPublicKey, ScriptBuf};

        let key = PublicKey::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(&[1; 32]).unwrap(),
        );
        let fingerprint = Fingerprint::from([1, 2, 3, 4]);
        let output = bitcoin::TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new_p2wpkh(&CompressedPublicKey(key).wpubkey_hash()),
        };
        let mut psbt = Psbt::from_unsigned_tx(bitcoin::Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn::default()],
            output: vec![output.clone()],
        })
        .unwrap();
        psbt.inputs[0].witness_utxo = Some(output);
        psbt.inputs[0]
            .bip32_derivation
            .insert(key, (fingerprint, "m/84'/0'/0'/0/0".parse().unwrap()));
        psbt.outputs[0]
            .bip32_derivation
            .insert(key, (fingerprint, "m/84'/0'/1'/1/3".parse().unwrap()));
        (psbt, fingerprint, key)
    }

    #[test]
    fn psbt_lowering_uses_firmware_aware_cross_account_outputs() {
        let (psbt, fingerprint, key) = psbt_fixture();
        let output_keypath = Keypath::try_from("m/84'/0'/1'/1/3").unwrap();
        for minor in [21, 22] {
            let version = semver::Version::new(9, minor, 0);
            let ordinary = Transaction::from_psbt(&version, &fingerprint[..], &psbt, None).unwrap();
            let resolved = Transaction::from_psbt_with_keys(
                &version,
                &psbt,
                None,
                vec![OurKey::Segwit(
                    key,
                    Keypath::try_from("m/84'/0'/0'/0/0").unwrap(),
                )],
                vec![Some(OurKey::Segwit(key, output_keypath.clone()))],
            )
            .unwrap();
            assert_eq!(ordinary.transaction, resolved.transaction);
            assert_eq!(
                ordinary.output_script_configs,
                resolved.output_script_configs
            );
            assert_eq!(
                ordinary.output_script_config_indices,
                resolved.output_script_config_indices
            );
            assert_eq!(resolved.transaction.script_configs.len(), 1);
            let validate = |version| {
                validate_signing(
                    &version,
                    &resolved.transaction,
                    &resolved.output_script_configs,
                    &resolved.output_script_config_indices,
                )
            };
            assert!(validate(version).is_ok());
            if minor == 21 {
                assert_eq!(resolved.output_script_config_indices, [None]);
                assert!(resolved.output_script_configs.is_empty());
                assert_eq!(
                    resolved.transaction.outputs[0],
                    TxOutput::External(
                        TxExternalOutput::try_from(&psbt.unsigned_tx.output[0]).unwrap()
                    ),
                );
            } else {
                assert_eq!(resolved.output_script_config_indices, [Some(0)]);
                assert_eq!(resolved.output_script_configs.len(), 1);
                assert!(matches!(
                    validate(semver::Version::new(9, 21, 0)),
                    Err(Error::Version(">=9.22.0"))
                ));
                assert_eq!(
                    resolved.output_script_configs[0].keypath,
                    Keypath::try_from("m/84'/0'/1'").unwrap().as_slice(),
                );
                assert_eq!(
                    resolved.transaction.outputs[0],
                    TxOutput::Internal(TxInternalOutput {
                        keypath: output_keypath.clone(),
                        value: 1000,
                        script_config_index: 0,
                    }),
                );
            }
        }
    }

    #[test]
    fn psbt_lowering_checks_psbt_and_key_cardinalities() {
        let (mut psbt, fingerprint, key) = psbt_fixture();
        let version = semver::Version::new(9, 24, 0);
        let input_keys = || {
            vec![OurKey::Segwit(
                key,
                Keypath::try_from("m/84'/0'/0'/0/0").unwrap(),
            )]
        };
        let input_error = "input signing keys must match PSBT inputs";
        let output_error = "output signing keys must match PSBT outputs";
        let expect_error = |result, expected: &str| {
            assert!(matches!(result, Err(Error::BtcSign(message)) if message == expected));
        };
        expect_error(
            Transaction::from_psbt_with_keys(&version, &psbt, None, vec![], vec![None]),
            input_error,
        );
        expect_error(
            Transaction::from_psbt_with_keys(&version, &psbt, None, input_keys(), vec![]),
            output_error,
        );
        // Even if key and metadata counts agree, the unsigned transaction must agree as well.
        psbt.unsigned_tx.input.push(bitcoin::TxIn::default());
        expect_error(
            Transaction::from_psbt_with_keys(&version, &psbt, None, input_keys(), vec![None]),
            input_error,
        );
        expect_error(
            Transaction::from_psbt(&version, &fingerprint[..], &psbt, None),
            input_error,
        );
        psbt.unsigned_tx.input.pop();
        psbt.unsigned_tx.output.clear();
        expect_error(
            Transaction::from_psbt_with_keys(&version, &psbt, None, input_keys(), vec![None]),
            output_error,
        );
        expect_error(
            Transaction::from_psbt(&version, &fingerprint[..], &psbt, None),
            output_error,
        );
    }

    #[test]
    fn psbt_lowering_checks_previous_output_index_without_a_witness_utxo() {
        let (mut psbt, fingerprint, key) = psbt_fixture();
        let version = semver::Version::new(9, 24, 0);
        let prev_tx = psbt.unsigned_tx.clone();
        psbt.unsigned_tx.input[0].previous_output =
            bitcoin::OutPoint::new(prev_tx.compute_txid(), 1);
        psbt.inputs[0].non_witness_utxo = Some(prev_tx);

        let lower = |psbt: &bitcoin::psbt::Psbt| {
            [
                Transaction::from_psbt(&version, &fingerprint[..], psbt, None),
                Transaction::from_psbt_with_keys(
                    &version,
                    psbt,
                    None,
                    vec![OurKey::Segwit(
                        key,
                        Keypath::try_from("m/84'/0'/0'/0/0").unwrap(),
                    )],
                    vec![None],
                ),
            ]
        };

        for result in lower(&psbt) {
            assert_eq!(result.unwrap().transaction.inputs[0].prev_out_value, 1000);
        }

        psbt.inputs[0].witness_utxo = None;
        for result in lower(&psbt) {
            assert!(matches!(
                result,
                Err(Error::Psbt(PsbtError::SignError(
                    SignError::MissingSpendUtxo
                )))
            ));
        }
    }

    #[test]
    fn psbt_lowering_requires_explicit_config_for_p2wsh() {
        use bitcoin::bip32::Xpriv;
        use bitcoin::hashes::Hash;
        use bitcoin::secp256k1::Secp256k1;

        let (mut psbt, fingerprint, _) = psbt_fixture();
        let version = semver::Version::new(9, 22, 0);
        let script = bitcoin::ScriptBuf::new_p2wsh(&bitcoin::WScriptHash::from_byte_array([5; 32]));
        psbt.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = script.clone();
        psbt.unsigned_tx.output[0].script_pubkey = script;
        assert!(matches!(
            Transaction::from_psbt(&version, &fingerprint[..], &psbt, None),
            Err(Error::Psbt(PsbtError::UnknownOutputType))
        ));
        let xpriv = Xpriv::new_master(bitcoin::Network::Bitcoin, &[7; 32]).unwrap();
        let forced = pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_policy(
                "wsh(pk(@0/**))",
                &[KeyOriginInfo {
                    root_fingerprint: Some(fingerprint),
                    keypath: None,
                    xpub: Xpub::from_priv(&Secp256k1::new(), &xpriv),
                }],
            )),
            keypath: vec![],
        };
        let lowered =
            Transaction::from_psbt(&version, &fingerprint[..], &psbt, Some(forced.clone()))
                .unwrap();
        assert_eq!(lowered.transaction.script_configs, [forced]);
        assert_eq!(lowered.transaction.inputs[0].script_config_index, 0);
        assert!(matches!(
            lowered.transaction.outputs[0],
            TxOutput::Internal(TxInternalOutput {
                script_config_index: 0,
                ..
            })
        ));
        assert!(lowered.output_script_configs.is_empty());
        assert_eq!(lowered.output_script_config_indices, [None]);
    }

    #[test]
    fn signature_application_checks_counts_before_mutating() {
        let (mut psbt, _, key) = psbt_fixture();
        let keys = [OurKey::Segwit(key, Keypath::try_from("m/").unwrap())];
        let mut compact = vec![0; 64];
        compact[31] = 1;
        compact[63] = 1;
        let signatures = [compact];
        let mut expect_error = |signatures: &[Vec<u8>], keys: &[OurKey]| {
            assert!(matches!(
                apply_signatures(&mut psbt, signatures, keys),
                Err(Error::BtcSign(message))
                    if message == "signatures and signing keys must match PSBT inputs"
            ));
            assert!(psbt.inputs[0].partial_sigs.is_empty());
        };
        expect_error(&[], &keys);
        expect_error(&signatures, &[]);

        // Matching signatures, keys, and PSBT metadata are still insufficient when the
        // underlying unsigned transaction has a different input count.
        psbt.unsigned_tx.input.push(bitcoin::TxIn::default());
        assert!(matches!(
            apply_signatures(&mut psbt, &signatures, &keys),
            Err(Error::BtcSign(message))
                if message == "signatures and signing keys must match PSBT inputs"
        ));
        assert!(psbt.inputs[0].partial_sigs.is_empty());
    }

    #[test]
    fn signature_application_rejects_invalid_ecdsa_scalars() {
        let (mut psbt, _, key) = psbt_fixture();
        let keys = [OurKey::Segwit(key, Keypath::try_from("m/").unwrap())];
        let mut zero_r = [0; 64];
        zero_r[63] = 1;
        let mut zero_s = [0; 64];
        zero_s[31] = 1;
        let mut high_s = zero_s;
        high_s[32..].copy_from_slice(&bitcoin::secp256k1::constants::CURVE_ORDER);
        high_s[63] -= 1;

        for invalid in [zero_r, zero_s, high_s] {
            assert!(matches!(
                apply_signatures(&mut psbt, &[invalid.to_vec()], &keys),
                Err(Error::InvalidSignature)
            ));
            assert!(psbt.inputs[0].partial_sigs.is_empty());
        }
    }

    #[test]
    fn signature_application_preserves_partial_signatures_and_applies_in_order() {
        use bitcoin::secp256k1::{Keypair, Message, PublicKey, Secp256k1, SecretKey};
        use bitcoin::taproot::{LeafVersion, TapLeafHash};

        let (mut psbt, _, key) = psbt_fixture();
        psbt.unsigned_tx
            .input
            .resize_with(3, bitcoin::TxIn::default);
        psbt.inputs.resize_with(3, Default::default);

        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1; 32]).unwrap();
        let cosigner = SecretKey::from_slice(&[2; 32]).unwrap();
        let cosigner_key = bitcoin::PublicKey::new(PublicKey::from_secret_key(&secp, &cosigner));
        let digest = Message::from_digest([3; 32]);
        let existing = bitcoin::ecdsa::Signature::sighash_all(secp.sign_ecdsa(&digest, &cosigner));
        psbt.inputs[0].partial_sigs.insert(cosigner_key, existing);

        let ecdsa = secp.sign_ecdsa(&digest, &secret);
        let schnorr =
            secp.sign_schnorr_no_aux_rand(&digest, &Keypair::from_secret_key(&secp, &secret));
        let taproot = bitcoin::taproot::Signature {
            signature: schnorr,
            sighash_type: bitcoin::sighash::TapSighashType::Default,
        };
        let xonly = key.x_only_public_key().0;
        let leaf = TapLeafHash::from_script(bitcoin::Script::new(), LeafVersion::TapScript);
        let path = Keypath::try_from("m/").unwrap();
        let keys = [
            OurKey::Segwit(key, path.clone()),
            OurKey::TaprootInternal(path.clone()),
            OurKey::TaprootScript(xonly, leaf, path),
        ];
        let mut signatures = [ecdsa.serialize_compact().to_vec(), vec![], taproot.to_vec()];

        assert!(matches!(
            apply_signatures(&mut psbt, &signatures, &keys),
            Err(Error::InvalidSignature)
        ));
        let our_pubkey = bitcoin::PublicKey::new(key);
        assert_eq!(
            psbt.inputs[0].partial_sigs[&our_pubkey],
            bitcoin::ecdsa::Signature::sighash_all(ecdsa),
        );
        assert!(psbt.inputs[1].tap_key_sig.is_none());
        assert!(psbt.inputs[2].tap_script_sigs.is_empty());

        signatures[1] = taproot.to_vec();
        apply_signatures(&mut psbt, &signatures, &keys).unwrap();
        assert_eq!(psbt.inputs[0].partial_sigs[&cosigner_key], existing);
        assert_eq!(psbt.inputs[0].partial_sigs.len(), 2);
        assert_eq!(psbt.inputs[1].tap_key_sig, Some(taproot));
        assert_eq!(psbt.inputs[2].tap_script_sigs[&(xonly, leaf)], taproot);
    }

    #[test]
    fn test_payload_from_pkscript() {
        use std::str::FromStr;
        // P2PKH
        let addr = bitcoin::Address::from_str("1AMZK8xzHJWsuRErpGZTiW4jKz8fdfLUGE")
            .unwrap()
            .assume_checked();
        let pkscript = addr.script_pubkey().into_bytes();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: pkscript[3..23].to_vec(),
                output_type: pb::BtcOutputType::P2pkh,
            }
        );

        // P2SH
        let addr = bitcoin::Address::from_str("3JFL8CgtV4ZtMFYeP5LgV4JppLkHw5Gw9T")
            .unwrap()
            .assume_checked();
        let pkscript = addr.script_pubkey().into_bytes();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: pkscript[2..22].to_vec(),
                output_type: pb::BtcOutputType::P2sh,
            }
        );

        // P2WPKH
        let addr = bitcoin::Address::from_str("bc1qkl8ms75cq6ajxtny7e88z3u9hkpkvktt5jwh6u")
            .unwrap()
            .assume_checked();
        let pkscript = addr.script_pubkey().into_bytes();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2wpkh,
            }
        );

        // P2WSH
        let addr = bitcoin::Address::from_str(
            "bc1q2fhgukymf0caaqrhfxrdju4wm94wwrch2ukntl5fuc0faz8zm49q0h6ss8",
        )
        .unwrap()
        .assume_checked();
        let pkscript = addr.script_pubkey().into_bytes();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2wsh,
            }
        );

        // P2TR
        let addr = bitcoin::Address::from_str(
            "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr",
        )
        .unwrap()
        .assume_checked();
        let pkscript = addr.script_pubkey().into_bytes();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: pkscript[2..].to_vec(),
                output_type: pb::BtcOutputType::P2tr,
            }
        );

        // OP_RETURN empty (OP_0)
        let pkscript = hex::decode("6a00").unwrap();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: Vec::new(),
                output_type: pb::BtcOutputType::OpReturn,
            }
        );

        // OP_RETURN with data push
        let pkscript = hex::decode("6a03aabbcc").unwrap();
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: vec![0xaa, 0xbb, 0xcc],
                output_type: pb::BtcOutputType::OpReturn,
            }
        );

        // OP_RETURN with 80-byte payload (PUSHDATA1)
        let mut pkscript = vec![opcodes::all::OP_RETURN.to_u8(), 0x4c, 0x50];
        pkscript.extend(std::iter::repeat_n(0xaa, 80));
        assert_eq!(
            Payload::from_pkscript(&pkscript).unwrap(),
            Payload {
                data: vec![0xaa; 80],
                output_type: pb::BtcOutputType::OpReturn,
            }
        );

        // Invalid OP_RETURN scripts
        let pkscript = hex::decode("6a").unwrap();
        assert!(matches!(
            Payload::from_pkscript(&pkscript),
            Err(PayloadError::InvalidOpReturn(
                "naked OP_RETURN is not supported"
            ))
        ));

        let pkscript = hex::decode("6a6a").unwrap();
        assert!(matches!(
            Payload::from_pkscript(&pkscript),
            Err(PayloadError::InvalidOpReturn(
                "no data push found after OP_RETURN"
            ))
        ));

        let pkscript = hex::decode("6a0000").unwrap();
        assert!(matches!(
            Payload::from_pkscript(&pkscript),
            Err(PayloadError::InvalidOpReturn(
                "only one data push supported after OP_RETURN"
            ))
        ));
    }
}
