// SPDX-License-Identifier: Apache-2.0

//! Functions and methods related to Bitcoin.

use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb::{self, request::Request, response::Response};
use crate::Keypath;
use crate::PairedBitBox;

pub use bitcoin::{
    bip32::{Fingerprint, Xpub},
    blockdata::script::witness_version::WitnessVersion,
    Script,
};

use bitcoin::blockdata::{opcodes, script::Instruction};

#[cfg(feature = "wasm")]
use enum_assoc::Assoc;

#[cfg(feature = "wasm")]
pub(crate) fn serde_deserialize_simple_type<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(pb::btc_script_config::SimpleType::deserialize(deserializer)?.into())
}

#[cfg(feature = "wasm")]
pub(crate) fn serde_deserialize_multisig<'de, D>(
    deserializer: D,
) -> Result<pb::btc_script_config::Multisig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    use std::str::FromStr;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Multisig {
        threshold: u32,
        xpubs: Vec<String>,
        our_xpub_index: u32,
        script_type: pb::btc_script_config::multisig::ScriptType,
    }
    let ms = Multisig::deserialize(deserializer)?;
    let xpubs = ms
        .xpubs
        .iter()
        .map(|s| Xpub::from_str(s.as_str()))
        .collect::<Result<Vec<Xpub>, _>>()
        .map_err(serde::de::Error::custom)?;
    Ok(pb::btc_script_config::Multisig {
        threshold: ms.threshold,
        xpubs: xpubs.iter().map(convert_xpub).collect(),
        our_xpub_index: ms.our_xpub_index,
        script_type: ms.script_type.into(),
    })
}

#[cfg(feature = "wasm")]
#[derive(serde::Deserialize)]
pub(crate) struct SerdeScriptConfig(pb::btc_script_config::Config);

#[cfg(feature = "wasm")]
impl From<SerdeScriptConfig> for pb::BtcScriptConfig {
    fn from(value: SerdeScriptConfig) -> Self {
        pb::BtcScriptConfig {
            config: Some(value.0),
        }
    }
}

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
    fn get_prev_tx(&self) -> Result<&PrevTx, Error> {
        self.prev_tx.as_ref().ok_or(Error::BtcSign(
            "input's previous transaction required but missing".into(),
        ))
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
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignMessageSignature {
    pub sig: Vec<u8>,
    pub recid: u8,
    pub electrum_sig65: Vec<u8>,
}

#[derive(thiserror::Error, Debug)]
#[cfg_attr(feature = "wasm", derive(Assoc), func(pub const fn js_code(&self) -> &'static str))]
pub enum PsbtError {
    #[error("{0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "sign-error"))]
    SignError(#[from] bitcoin::psbt::SignError),
    #[error("Taproot pubkeys must be unique across the internal key and all leaf scripts.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "key-not-unique"))]
    KeyNotUnique,
    #[error("Could not find our key in an input.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "key-not-found"))]
    KeyNotFound,
    #[error("Unrecognized/unsupported output type.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unknown-output-type"))]
    UnknownOutputType,
    #[error("Invalid OP_RETURN script: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "invalid-op-return"))]
    InvalidOpReturn(&'static str),
    #[error("Account script configs must contain a BIP44 account keypath.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "invalid-account-keypath"))]
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

enum OurKey {
    Segwit(bitcoin::secp256k1::PublicKey, Keypath),
    TaprootInternal(Keypath),
    TaprootScript(
        bitcoin::secp256k1::XOnlyPublicKey,
        bitcoin::taproot::TapLeafHash,
        Keypath,
    ),
}

impl OurKey {
    fn keypath(&self) -> Keypath {
        match self {
            OurKey::Segwit(_, kp) => kp.clone(),
            OurKey::TaprootInternal(kp) => kp.clone(),
            OurKey::TaprootScript(_, _, kp) => kp.clone(),
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
    keypath: Keypath,
    redeem_script: Option<&bitcoin::ScriptBuf>,
    _witness_script: Option<&bitcoin::ScriptBuf>,
) -> Result<pb::BtcScriptConfigWithKeypath, PsbtError> {
    let keypath = keypath.hardened_prefix();
    if output.script_pubkey.is_p2wpkh() {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2wpkh,
            )),
            keypath: keypath.to_vec(),
        });
    }
    let redeem_script_is_p2wpkh = redeem_script.map(|s| s.is_p2wpkh()).unwrap_or(false);
    if output.script_pubkey.is_p2sh() && redeem_script_is_p2wpkh {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2wpkhP2sh,
            )),
            keypath: keypath.to_vec(),
        });
    }
    if output.script_pubkey.is_p2tr() {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2tr,
            )),
            keypath: keypath.to_vec(),
        });
    }
    // Check for segwit multisig (p2wsh or p2wsh-p2sh).
    let redeem_script_is_p2wsh = redeem_script.map(|s| s.is_p2wsh()).unwrap_or(false);
    let is_p2wsh_p2sh = output.script_pubkey.is_p2sh() && redeem_script_is_p2wsh;
    if output.script_pubkey.is_p2wsh() || is_p2wsh_p2sh {
        todo!();
    }
    Err(PsbtError::UnknownOutputType)
}

struct PsbtTransaction {
    transaction: Transaction,
    our_keys: Vec<OurKey>,
    output_script_configs: Vec<pb::BtcScriptConfigWithKeypath>,
    output_script_config_indices: Vec<Option<u32>>,
}

impl Transaction {
    fn from_psbt(
        firmware_version: &semver::Version,
        our_root_fingerprint: &[u8],
        psbt: &bitcoin::psbt::Psbt,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
    ) -> Result<PsbtTransaction, PsbtError> {
        let is_script_config_forced = force_script_config.is_some();
        let mut script_configs = force_script_config.into_iter().collect::<Vec<_>>();
        let mut output_script_configs: Vec<pb::BtcScriptConfigWithKeypath> = Vec::new();

        let mut our_keys: Vec<OurKey> = Vec::new();
        let mut inputs: Vec<TxInput> = Vec::new();

        for (input_index, (tx_input, psbt_input)) in
            psbt.unsigned_tx.input.iter().zip(&psbt.inputs).enumerate()
        {
            let utxo = psbt.spend_utxo(input_index)?;
            let our_key = find_our_key(our_root_fingerprint, psbt_input)?;
            let script_config_index = if is_script_config_forced {
                0
            } else {
                find_or_add_script_config(
                    &mut script_configs,
                    script_config_from_utxo(
                        utxo,
                        our_key.keypath(),
                        psbt_input.redeem_script.as_ref(),
                        psbt_input.witness_script.as_ref(),
                    )?,
                )
            };

            inputs.push(TxInput {
                prev_out_hash: (tx_input.previous_output.txid.as_ref() as &[u8]).to_vec(),
                prev_out_index: tx_input.previous_output.vout,
                prev_out_value: utxo.value.to_sat(),
                sequence: tx_input.sequence.to_consensus_u32(),
                keypath: our_key.keypath(),
                script_config_index: script_config_index as _,
                prev_tx: psbt_input.non_witness_utxo.as_ref().map(PrevTx::from),
            });
            our_keys.push(our_key);
        }

        let mut outputs: Vec<TxOutput> = Vec::new();
        let mut output_script_config_indices: Vec<Option<u32>> = Vec::new();
        for (tx_output, psbt_output) in psbt.unsigned_tx.output.iter().zip(&psbt.outputs) {
            let our_key = find_our_key(our_root_fingerprint, psbt_output);
            // Either change output or a non-change output owned by the BitBox.
            match our_key {
                Ok(our_key) => {
                    let script_config = if is_script_config_forced {
                        script_configs[0].clone()
                    } else {
                        script_config_from_utxo(
                            tx_output,
                            our_key.keypath(),
                            psbt_output.redeem_script.as_ref(),
                            psbt_output.witness_script.as_ref(),
                        )?
                    };
                    let same_account = is_same_account(&script_configs, &script_config)?;
                    // Firmware 9.22 added separate output configs for device-owned outputs that
                    // belong to another account. Older firmware must display them as external.
                    if same_account {
                        let script_config_index =
                            find_or_add_script_config(&mut script_configs, script_config);
                        outputs.push(TxOutput::Internal(TxInternalOutput {
                            keypath: our_key.keypath(),
                            value: tx_output.value.to_sat(),
                            script_config_index: script_config_index as _,
                        }));
                        output_script_config_indices.push(None);
                    } else if firmware_version >= &semver::Version::new(9, 22, 0) {
                        let output_script_config_index =
                            find_or_add_script_config(&mut output_script_configs, script_config);
                        outputs.push(TxOutput::Internal(TxInternalOutput {
                            keypath: our_key.keypath(),
                            value: tx_output.value.to_sat(),
                            // Ignored when output_script_config_index is present.
                            script_config_index: 0,
                        }));
                        output_script_config_indices.push(Some(output_script_config_index as _));
                    } else {
                        outputs.push(TxOutput::External(tx_output.try_into()?));
                        output_script_config_indices.push(None);
                    }
                }
                Err(_) => {
                    outputs.push(TxOutput::External(tx_output.try_into()?));
                    output_script_config_indices.push(None);
                }
            }
        }

        Ok(PsbtTransaction {
            transaction: Transaction {
                script_configs,
                version: psbt.unsigned_tx.version.0 as _,
                inputs,
                outputs,
                locktime: psbt.unsigned_tx.lock_time.to_consensus_u32(),
            },
            our_keys,
            output_script_configs,
            output_script_config_indices,
        })
    }
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

#[derive(Clone)]
#[cfg_attr(
    feature = "wasm",
    derive(serde::Deserialize),
    serde(rename_all = "camelCase")
)]
#[derive(PartialEq)]
pub struct KeyOriginInfo {
    pub root_fingerprint: Option<bitcoin::bip32::Fingerprint>,
    pub keypath: Option<Keypath>,
    pub xpub: bitcoin::bip32::Xpub,
}

fn convert_xpub(xpub: &bitcoin::bip32::Xpub) -> pb::XPub {
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

fn is_schnorr(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
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
        match self
            .query_proto(Request::BtcPub(pb::BtcPubRequest {
                coin: coin as _,
                keypath: keypath.to_vec(),
                display,
                output: Some(pb::btc_pub_request::Output::XpubType(xpub_type as _)),
            }))
            .await?
        {
            Response::Pub(pb::PubResponse { r#pub }) => Ok(r#pub),
            _ => Err(Error::UnexpectedResponse),
        }
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
        match self
            .query_proto(Request::BtcPub(pb::BtcPubRequest {
                coin: coin as _,
                keypath: keypath.to_vec(),
                display,
                output: Some(pb::btc_pub_request::Output::ScriptConfig(
                    script_config.clone(),
                )),
            }))
            .await?
        {
            Response::Pub(pb::PubResponse { r#pub }) => Ok(r#pub),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    async fn query_proto_btc(
        &self,
        request: pb::btc_request::Request,
    ) -> Result<pb::btc_response::Response, Error> {
        match self
            .query_proto(Request::Btc(pb::BtcRequest {
                request: Some(request),
            }))
            .await?
        {
            Response::Btc(pb::BtcResponse {
                response: Some(response),
            }) => Ok(response),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    async fn get_next_response(&self, request: Request) -> Result<pb::BtcSignNextResponse, Error> {
        match self.query_proto(request).await? {
            Response::BtcSignNext(next_response) => Ok(next_response),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    async fn get_next_response_nested(
        &self,
        request: pb::btc_request::Request,
    ) -> Result<pb::BtcSignNextResponse, Error> {
        match self.query_proto_btc(request).await? {
            pb::btc_response::Response::SignNext(next_response) => Ok(next_response),
            _ => Err(Error::UnexpectedResponse),
        }
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
        if !output_script_config_indices.is_empty()
            && output_script_config_indices.len() != transaction.outputs.len()
        {
            return Err(Error::BtcSign(
                "output script config indices must match transaction outputs".into(),
            ));
        }
        self.validate_version(">=9.4.0")?; // anti-klepto since 9.4.0
        if transaction
            .script_configs
            .iter()
            .chain(output_script_configs)
            .any(is_taproot_simple)
        {
            self.validate_version(">=9.10.0")?; // taproot since 9.10.0
        }
        if transaction.outputs.iter().any(|output| {
            matches!(
                output,
                TxOutput::External(tx_output)
                    if tx_output.payload.output_type == pb::BtcOutputType::OpReturn
            )
        }) {
            self.validate_version(">=9.24.0")?;
        }

        let mut sigs: Vec<Vec<u8>> = Vec::new();

        let mut next_response = self
            .get_next_response(Request::BtcSignInit(pb::BtcSignInitRequest {
                coin: coin as _,
                script_configs: transaction.script_configs.clone(),
                output_script_configs: output_script_configs.to_vec(),
                version: transaction.version,
                num_inputs: transaction.inputs.len() as _,
                num_outputs: transaction.outputs.len() as _,
                locktime: transaction.locktime,
                format_unit: format_unit as _,
                contains_silent_payment_outputs: false,
            }))
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
                        .get_next_response(Request::BtcSignInput(pb::BtcSignInputRequest {
                            prev_out_hash: tx_input.prev_out_hash.clone(),
                            prev_out_index: tx_input.prev_out_index,
                            prev_out_value: tx_input.prev_out_value,
                            sequence: tx_input.sequence,
                            keypath: tx_input.keypath.to_vec(),
                            script_config_index: tx_input.script_config_index,
                            host_nonce_commitment: host_nonce.as_ref().map(|host_nonce| {
                                pb::AntiKleptoHostNonceCommitment {
                                    commitment: crate::antiklepto::host_commit(host_nonce).to_vec(),
                                }
                            }),
                        }))
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
                                .get_next_response_nested(
                                    pb::btc_request::Request::AntikleptoSignature(
                                        pb::AntiKleptoSignatureRequest {
                                            host_nonce: host_nonce.to_vec(),
                                        },
                                    ),
                                )
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
                        sigs.push(next_response.signature.clone());
                    }
                    if input_index == transaction.inputs.len() - 1 {
                        is_inputs_pass2 = true
                    }
                }
                pb::btc_sign_next_response::Type::PrevtxInit => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    next_response = self
                        .get_next_response_nested(pb::btc_request::Request::PrevtxInit(
                            pb::BtcPrevTxInitRequest {
                                version: prevtx.version,
                                num_inputs: prevtx.inputs.len() as _,
                                num_outputs: prevtx.outputs.len() as _,
                                locktime: prevtx.locktime,
                            },
                        ))
                        .await?;
                }
                pb::btc_sign_next_response::Type::PrevtxInput => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    let prevtx_input: &PrevTxInput =
                        &prevtx.inputs[next_response.prev_index as usize];
                    next_response = self
                        .get_next_response_nested(pb::btc_request::Request::PrevtxInput(
                            pb::BtcPrevTxInputRequest {
                                prev_out_hash: prevtx_input.prev_out_hash.clone(),
                                prev_out_index: prevtx_input.prev_out_index,
                                signature_script: prevtx_input.signature_script.clone(),
                                sequence: prevtx_input.sequence,
                            },
                        ))
                        .await?;
                }
                pb::btc_sign_next_response::Type::PrevtxOutput => {
                    let prevtx: &PrevTx =
                        transaction.inputs[next_response.index as usize].get_prev_tx()?;
                    let prevtx_output: &PrevTxOutput =
                        &prevtx.outputs[next_response.prev_index as usize];
                    next_response = self
                        .get_next_response_nested(pb::btc_request::Request::PrevtxOutput(
                            pb::BtcPrevTxOutputRequest {
                                value: prevtx_output.value,
                                pubkey_script: prevtx_output.pubkey_script.clone(),
                            },
                        ))
                        .await?;
                }
                pb::btc_sign_next_response::Type::Output => {
                    let output_index = next_response.index as usize;
                    let tx_output: &TxOutput = &transaction.outputs[output_index];
                    let output_script_config_index = output_script_config_indices
                        .get(output_index)
                        .copied()
                        .flatten();
                    let request: Request = match tx_output {
                        TxOutput::Internal(output) => {
                            Request::BtcSignOutput(pb::BtcSignOutputRequest {
                                ours: true,
                                value: output.value,
                                keypath: output.keypath.to_vec(),
                                script_config_index: output.script_config_index,
                                output_script_config_index,
                                ..Default::default()
                            })
                        }
                        TxOutput::External(output) => {
                            Request::BtcSignOutput(pb::BtcSignOutputRequest {
                                ours: false,
                                value: output.value,
                                r#type: output.payload.output_type as _,
                                payload: output.payload.data.clone(),
                                ..Default::default()
                            })
                        }
                    };
                    next_response = self.get_next_response(request).await?;
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
        for (psbt_input, (signature, our_key)) in psbt
            .inputs
            .iter_mut()
            .zip(signatures.iter().zip(psbt_transaction.our_keys))
        {
            match our_key {
                OurKey::Segwit(pubkey, _) => {
                    psbt_input.partial_sigs.insert(
                        bitcoin::PublicKey::new(pubkey),
                        bitcoin::ecdsa::Signature {
                            signature: bitcoin::secp256k1::ecdsa::Signature::from_compact(
                                signature,
                            )
                            .map_err(|_| Error::InvalidSignature)?,
                            sighash_type: bitcoin::sighash::EcdsaSighashType::All,
                        },
                    );
                }
                OurKey::TaprootInternal(_) => {
                    psbt_input.tap_key_sig = Some(
                        bitcoin::taproot::Signature::from_slice(signature)
                            .map_err(|_| Error::InvalidSignature)?,
                    );
                }
                OurKey::TaprootScript(xonly, leaf_hash, _) => {
                    let sig = bitcoin::taproot::Signature::from_slice(signature)
                        .map_err(|_| Error::InvalidSignature)?;
                    psbt_input.tap_script_sigs.insert((xonly, leaf_hash), sig);
                }
            }
        }
        Ok(())
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
        let request = pb::BtcSignMessageRequest {
            coin: coin as _,
            script_config: Some(script_config),
            msg: msg.to_vec(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
            }),
        };

        let response = self
            .query_proto_btc(pb::btc_request::Request::SignMessage(request))
            .await?;
        let signer_commitment = match response {
            pb::btc_response::Response::AntikleptoSignerCommitment(
                pb::AntiKleptoSignerCommitment { commitment },
            ) => commitment,
            _ => return Err(Error::UnexpectedResponse),
        };

        let request = pb::AntiKleptoSignatureRequest {
            host_nonce: host_nonce.to_vec(),
        };

        let response = self
            .query_proto_btc(pb::btc_request::Request::AntikleptoSignature(request))
            .await?;
        let signature = match response {
            pb::btc_response::Response::SignMessage(pb::BtcSignMessageResponse { signature }) => {
                signature
            }
            _ => return Err(Error::UnexpectedResponse),
        };
        crate::antiklepto::verify_ecdsa(&host_nonce, &signer_commitment, &signature)?;

        let sig = signature[..64].to_vec();
        let recid = signature[64];
        let compressed: u8 = 4; // BitBox02 uses only compressed pubkeys
        let sig65: u8 = 27 + compressed + recid;
        let mut electrum_sig65 = vec![sig65];
        electrum_sig65.extend_from_slice(&sig);
        Ok(SignMessageSignature {
            sig,
            recid,
            electrum_sig65,
        })
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
        match self
            .query_proto_btc(pb::btc_request::Request::IsScriptConfigRegistered(
                pb::BtcIsScriptConfigRegisteredRequest {
                    registration: Some(pb::BtcScriptConfigRegistration {
                        coin: coin as _,
                        script_config: Some(script_config.clone()),
                        keypath: keypath_account.map_or(vec![], |kp| kp.to_vec()),
                    }),
                },
            ))
            .await?
        {
            pb::btc_response::Response::IsScriptConfigRegistered(response) => {
                Ok(response.is_registered)
            }
            _ => Err(Error::UnexpectedResponse),
        }
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
        match self
            .query_proto_btc(pb::btc_request::Request::RegisterScriptConfig(
                pb::BtcRegisterScriptConfigRequest {
                    registration: Some(pb::BtcScriptConfigRegistration {
                        coin: coin as _,
                        script_config: Some(script_config.clone()),
                        keypath: keypath_account.map_or(vec![], |kp| kp.to_vec()),
                    }),
                    name: name.unwrap_or("").into(),
                    xpub_type: xpub_type as _,
                },
            ))
            .await?
        {
            pb::btc_response::Response::Success(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
