//! Functions and methods related to Bitcoin.

use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb::{self, request::Request, response::Response};
use crate::Keypath;
use crate::PairedBitBox;

pub use bitcoin::bip32::{ExtendedPubKey, Fingerprint};

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
            value: value.value,
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
            version: value.version as _,
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
    #[error("{0}")]
    AddressError(#[from] bitcoin::address::Error),
    #[error("invalid witness program size")]
    InvalidWitnessProgramSize,
    #[error("witness version {0} not supported yet")]
    UnsupportedWitnessVersion(bitcoin::address::WitnessVersion),
    #[error("unrecognized pubkey script")]
    Unrecognized,
}

impl TryFrom<bitcoin::address::Payload> for Payload {
    type Error = PayloadError;
    fn try_from(value: bitcoin::address::Payload) -> Result<Self, Self::Error> {
        match value {
            bitcoin::address::Payload::PubkeyHash(h) => Ok(Payload {
                data: h[..].to_vec(),
                output_type: pb::BtcOutputType::P2pkh,
            }),
            bitcoin::address::Payload::ScriptHash(h) => Ok(Payload {
                data: h[..].to_vec(),
                output_type: pb::BtcOutputType::P2sh,
            }),
            bitcoin::address::Payload::WitnessProgram(w) => match w.version() {
                bitcoin::address::WitnessVersion::V0 => Ok(Payload {
                    data: w.program().as_bytes().to_vec(),
                    output_type: match w.program().len() {
                        20 => pb::BtcOutputType::P2wpkh,
                        32 => pb::BtcOutputType::P2wsh,
                        _ => return Err(PayloadError::InvalidWitnessProgramSize),
                    },
                }),
                bitcoin::address::WitnessVersion::V1 => match w.program().len() {
                    32 => Ok(Payload {
                        data: w.program().as_bytes().to_vec(),
                        output_type: pb::BtcOutputType::P2tr,
                    }),
                    _ => Err(PayloadError::InvalidWitnessProgramSize),
                },
                version => Err(PayloadError::UnsupportedWitnessVersion(version)),
            },
            _ => Err(PayloadError::Unrecognized),
        }
    }
}

impl Payload {
    pub fn from_pkscript(pkscript: &[u8]) -> Result<Payload, PayloadError> {
        let payload =
            bitcoin::address::Payload::from_script(bitcoin::Script::from_bytes(pkscript))?;
        payload.try_into()
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
            payload: Payload::from_pkscript(value.script_pubkey.as_bytes())
                .map_err(|_| PsbtError::UnknownOutputType)?,
            value: value.value,
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

#[derive(thiserror::Error, Debug)]
#[cfg_attr(feature = "wasm", derive(Assoc), func(pub const fn js_code(&self) -> &'static str))]
pub enum PsbtError {
    #[error("{0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "sign-error"))]
    SignError(#[from] bitcoin::psbt::SignError),
    #[error("The BitBox does not support Taproot script path spending.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unsupported-tap-script"))]
    UnsupportedTapScript,
    #[error("Could not find our key in an input.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "key-not-found"))]
    KeyNotFound,
    #[error("Unrecognized/unsupported output type.")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unknown-output-type"))]
    UnknownOutputType,
}

enum OurKey {
    Segwit(bitcoin::secp256k1::PublicKey, Keypath),
    TaprootInternal(Keypath),
}

impl OurKey {
    fn keypath(&self) -> Keypath {
        match self {
            OurKey::Segwit(_, kp) => kp.clone(),
            OurKey::TaprootInternal(kp) => kp.clone(),
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
    if let Some(tap_internal_key) = output_info.get_tap_internal_key() {
        let (leaf_hashes, (fingerprint, derivation_path)) = output_info
            .get_tap_key_origins()
            .get(tap_internal_key)
            .ok_or(PsbtError::KeyNotFound)?;
        if !leaf_hashes.is_empty() {
            return Err(PsbtError::UnsupportedTapScript);
        }
        if &fingerprint[..] == our_root_fingerprint {
            // TODO: check for fingerprint collision
            return Ok(OurKey::TaprootInternal(derivation_path.into()));
        }
        return Err(PsbtError::KeyNotFound);
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
    if output.script_pubkey.is_v0_p2wpkh() {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2wpkh,
            )),
            keypath: keypath.to_vec(),
        });
    }
    let redeem_script_is_p2wpkh = redeem_script.map(|s| s.is_v0_p2wpkh()).unwrap_or(false);
    if output.script_pubkey.is_p2sh() && redeem_script_is_p2wpkh {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2wpkhP2sh,
            )),
            keypath: keypath.to_vec(),
        });
    }
    if output.script_pubkey.is_v1_p2tr() {
        return Ok(pb::BtcScriptConfigWithKeypath {
            script_config: Some(make_script_config_simple(
                pb::btc_script_config::SimpleType::P2tr,
            )),
            keypath: keypath.to_vec(),
        });
    }
    // Check for segwit multisig (p2wsh or p2wsh-p2sh).
    let redeem_script_is_p2wsh = redeem_script.map(|s| s.is_v0_p2wsh()).unwrap_or(false);
    let is_p2wsh_p2sh = output.script_pubkey.is_p2sh() && redeem_script_is_p2wsh;
    if output.script_pubkey.is_v0_p2wsh() || is_p2wsh_p2sh {
        todo!();
    }
    Err(PsbtError::UnknownOutputType)
}

impl Transaction {
    fn from_psbt(
        our_root_fingerprint: &[u8],
        psbt: &bitcoin::psbt::PartiallySignedTransaction,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
    ) -> Result<(Self, Vec<OurKey>), PsbtError> {
        let mut script_configs: Vec<pb::BtcScriptConfigWithKeypath> = Vec::new();
        let mut is_script_config_forced = false;
        if let Some(cfg) = force_script_config {
            script_configs.push(cfg);
            is_script_config_forced = true;
        }

        let mut our_keys: Vec<OurKey> = Vec::new();
        let mut inputs: Vec<TxInput> = Vec::new();

        let mut add_script_config = |script_config: pb::BtcScriptConfigWithKeypath| -> usize {
            match script_configs.iter().position(|el| el == &script_config) {
                Some(pos) => pos,
                None => {
                    script_configs.push(script_config);
                    script_configs.len() - 1
                }
            }
        };

        for (input_index, (tx_input, psbt_input)) in
            psbt.unsigned_tx.input.iter().zip(&psbt.inputs).enumerate()
        {
            let utxo = psbt.spend_utxo(input_index)?;
            let our_key = find_our_key(our_root_fingerprint, psbt_input)?;
            let script_config_index = if is_script_config_forced {
                0
            } else {
                add_script_config(script_config_from_utxo(
                    utxo,
                    our_key.keypath(),
                    psbt_input.redeem_script.as_ref(),
                    psbt_input.witness_script.as_ref(),
                )?)
            };

            inputs.push(TxInput {
                prev_out_hash: (tx_input.previous_output.txid.as_ref() as &[u8]).to_vec(),
                prev_out_index: tx_input.previous_output.vout,
                prev_out_value: utxo.value,
                sequence: tx_input.sequence.to_consensus_u32(),
                keypath: our_key.keypath(),
                script_config_index: script_config_index as _,
                prev_tx: psbt_input.non_witness_utxo.as_ref().map(PrevTx::from),
            });
            our_keys.push(our_key);
        }

        let mut outputs: Vec<TxOutput> = Vec::new();
        for (tx_output, psbt_output) in psbt.unsigned_tx.output.iter().zip(&psbt.outputs) {
            let our_key = find_our_key(our_root_fingerprint, psbt_output);
            // Either change output or a non-change output owned by the BitBox.
            match our_key {
                Ok(our_key) => {
                    let script_config_index = if is_script_config_forced {
                        0
                    } else {
                        add_script_config(script_config_from_utxo(
                            tx_output,
                            our_key.keypath(),
                            psbt_output.redeem_script.as_ref(),
                            psbt_output.witness_script.as_ref(),
                        )?)
                    };
                    outputs.push(TxOutput::Internal(TxInternalOutput {
                        keypath: our_key.keypath(),
                        value: tx_output.value,
                        script_config_index: script_config_index as _,
                    }));
                }
                Err(_) => {
                    outputs.push(TxOutput::External(tx_output.try_into()?));
                }
            }
        }

        Ok((
            Transaction {
                script_configs,
                version: psbt.unsigned_tx.version as _,
                inputs,
                outputs,
                locktime: psbt.unsigned_tx.lock_time.to_consensus_u32(),
            },
            our_keys,
        ))
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
    pub xpub: bitcoin::bip32::ExtendedPubKey,
}

fn convert_xpub(xpub: &bitcoin::bip32::ExtendedPubKey) -> pb::XPub {
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

fn is_taproot(script_config: &pb::BtcScriptConfigWithKeypath) -> bool {
    matches!(script_config,
        pb::BtcScriptConfigWithKeypath {
            script_config:
                Some(pb::BtcScriptConfig {
                    config: Some(pb::btc_script_config::Config::SimpleType(simple_type)),
                }),
            ..
        } if *simple_type == pb::btc_script_config::SimpleType::P2tr as i32)
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
        self.validate_version(">=9.4.0")?; // anti-klepto since 9.4.0
        if transaction.script_configs.iter().any(is_taproot) {
            self.validate_version(">=9.10.0")?; // taproot since 9.10.0
        }

        let mut sigs: Vec<Vec<u8>> = Vec::new();

        let mut next_response = self
            .get_next_response(Request::BtcSignInit(pb::BtcSignInitRequest {
                coin: coin as _,
                script_configs: transaction.script_configs.clone(),
                version: transaction.version,
                num_inputs: transaction.inputs.len() as _,
                num_outputs: transaction.outputs.len() as _,
                locktime: transaction.locktime,
                format_unit: format_unit as _,
            }))
            .await?;

        let mut is_inputs_pass2 = false;
        loop {
            match pb::btc_sign_next_response::Type::from_i32(next_response.r#type)
                .ok_or(Error::UnexpectedResponse)?
            {
                pb::btc_sign_next_response::Type::Input => {
                    let input_index: usize = next_response.index as _;
                    let tx_input: &TxInput = &transaction.inputs[input_index];

                    let input_is_schnorr = is_taproot(
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
                    // TODO handle error
                    let prevtx: &PrevTx = transaction.inputs[next_response.index as usize]
                        .prev_tx
                        .as_ref()
                        .unwrap();
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
                    // TODO handle error
                    let prevtx: &PrevTx = transaction.inputs[next_response.index as usize]
                        .prev_tx
                        .as_ref()
                        .unwrap();
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
                    // TODO handle error
                    let prevtx: &PrevTx = transaction.inputs[next_response.index as usize]
                        .prev_tx
                        .as_ref()
                        .unwrap();
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
                    let tx_output: &TxOutput = &transaction.outputs[next_response.index as usize];
                    let request: Request = match tx_output {
                        TxOutput::Internal(output) => {
                            Request::BtcSignOutput(pb::BtcSignOutputRequest {
                                ours: true,
                                value: output.value,
                                keypath: output.keypath.to_vec(),
                                script_config_index: output.script_config_index,
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
        psbt: &mut bitcoin::psbt::PartiallySignedTransaction,
        force_script_config: Option<pb::BtcScriptConfigWithKeypath>,
        format_unit: pb::btc_sign_init_request::FormatUnit,
    ) -> Result<(), Error> {
        // since v9.15.0, the BitBox02 accepts "internal" outputs (ones sent to the BitBox02 with
        // the keypath) even if the keypath is not a change keypath. PSBTs often contain the key
        // origin info in outputs even in regular send-to-self outputs.
        self.validate_version(">=9.15.0")?;

        let our_root_fingerprint = hex::decode(&self.root_fingerprint().await?).unwrap();
        let (transaction, our_keys) =
            Transaction::from_psbt(&our_root_fingerprint, psbt, force_script_config)?;
        let signatures = self.btc_sign(coin, &transaction, format_unit).await?;
        for (psbt_input, (signature, our_key)) in
            psbt.inputs.iter_mut().zip(signatures.iter().zip(our_keys))
        {
            match our_key {
                OurKey::Segwit(pubkey, _) => {
                    psbt_input.partial_sigs.insert(
                        bitcoin::PublicKey::new(pubkey),
                        bitcoin::ecdsa::Signature {
                            sig: bitcoin::secp256k1::ecdsa::Signature::from_compact(signature)
                                .map_err(|_| Error::InvalidSignature)?,
                            hash_ty: bitcoin::sighash::EcdsaSighashType::All,
                        },
                    );
                }
                OurKey::TaprootInternal(_) => {
                    psbt_input.tap_key_sig = Some(
                        bitcoin::taproot::Signature::from_slice(signature)
                            .map_err(|_| Error::InvalidSignature)?,
                    )
                }
            }
        }
        Ok(())
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
    use crate::keypath::HARDENED;

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
    }

    // Test that a PSBT containing only p2wpkh inputs is converted correctly to a transaction to be
    // signed by the BitBox.
    #[test]
    fn test_transaction_from_psbt_p2wpkh() {
        use std::str::FromStr;

        // Based on mnemonic:
        // route glue else try obey local kidney future teach unaware pulse exclude.
        let psbt_str = "cHNidP8BAHECAAAAAfbXTun4YYxDroWyzRq3jDsWFVlsZ7HUzxiORY/iR4goAAAAAAD9////AuLCAAAAAAAAFgAUg3w5W0zt3AmxRmgA5Q6wZJUDRhUowwAAAAAAABYAFJjQqUoXDcwUEqfExu9pnaSn5XBct0ElAAABAR+ghgEAAAAAABYAFHn03igII+hp819N2Zlb5LnN8atRAQDfAQAAAAABAZ9EJlMJnXF5bFVrb1eFBYrEev3pg35WpvS3RlELsMMrAQAAAAD9////AqCGAQAAAAAAFgAUefTeKAgj6GnzX03ZmVvkuc3xq1EoRs4JAAAAABYAFKG2PzjYjknaA6lmXFqPaSgHwXX9AkgwRQIhAL0v0r3LisQ9KOlGzMhM/xYqUmrv2a5sORRlkX1fqDC8AiB9XqxSNEdb4mPnp7ylF1cAlbAZ7jMhgIxHUXylTww3bwEhA0AEOM0yYEpexPoKE3vT51uxZ+8hk9sOEfBFKOeo6oDDAAAAACIGAyNQfmAT/YLmZaxxfDwClmVNt2BkFnfQu/i8Uc/hHDUiGBKiwYlUAACAAQAAgAAAAIAAAAAAAAAAAAAAIgIDnxFM7Qr9LvJwQDB9GozdTRIe3MYVuHOqT7dU2EuvHrIYEqLBiVQAAIABAACAAAAAgAEAAAAAAAAAAA==";

        let expected_transaction = Transaction {
            script_configs: vec![pb::BtcScriptConfigWithKeypath {
                script_config: Some(pb::BtcScriptConfig {
                    config: Some(pb::btc_script_config::Config::SimpleType(
                        pb::btc_script_config::SimpleType::P2wpkh as _,
                    )),
                }),
                keypath: vec![84 + HARDENED, 1 + HARDENED, HARDENED],
            }],
            version: 2,
            inputs: vec![TxInput {
                prev_out_hash: vec![
                    246, 215, 78, 233, 248, 97, 140, 67, 174, 133, 178, 205, 26, 183, 140, 59, 22,
                    21, 89, 108, 103, 177, 212, 207, 24, 142, 69, 143, 226, 71, 136, 40,
                ],
                prev_out_index: 0,
                prev_out_value: 100000,
                sequence: 4294967293,
                keypath: "m/84'/1'/0'/0/0".try_into().unwrap(),
                script_config_index: 0,
                prev_tx: Some(PrevTx {
                    version: 1,
                    inputs: vec![PrevTxInput {
                        prev_out_hash: vec![
                            159, 68, 38, 83, 9, 157, 113, 121, 108, 85, 107, 111, 87, 133, 5, 138,
                            196, 122, 253, 233, 131, 126, 86, 166, 244, 183, 70, 81, 11, 176, 195,
                            43,
                        ],
                        prev_out_index: 1,
                        signature_script: vec![],
                        sequence: 4294967293,
                    }],
                    outputs: vec![
                        PrevTxOutput {
                            value: 100000,
                            pubkey_script: vec![
                                0, 20, 121, 244, 222, 40, 8, 35, 232, 105, 243, 95, 77, 217, 153,
                                91, 228, 185, 205, 241, 171, 81,
                            ],
                        },
                        PrevTxOutput {
                            value: 164513320,
                            pubkey_script: vec![
                                0, 20, 161, 182, 63, 56, 216, 142, 73, 218, 3, 169, 102, 92, 90,
                                143, 105, 40, 7, 193, 117, 253,
                            ],
                        },
                    ],
                    locktime: 0,
                }),
            }],
            outputs: vec![
                TxOutput::External(TxExternalOutput {
                    payload: Payload {
                        data: vec![
                            131, 124, 57, 91, 76, 237, 220, 9, 177, 70, 104, 0, 229, 14, 176, 100,
                            149, 3, 70, 21,
                        ],
                        output_type: pb::BtcOutputType::P2wpkh,
                    },
                    value: 49890,
                }),
                TxOutput::Internal(TxInternalOutput {
                    keypath: "m/84'/1'/0'/1/0".try_into().unwrap(),
                    value: 49960,
                    script_config_index: 0,
                }),
            ],
            locktime: 2441655,
        };
        let our_root_fingerprint = hex::decode("12a2c189").unwrap();
        let psbt = bitcoin::psbt::PartiallySignedTransaction::from_str(psbt_str).unwrap();
        let (transaction, _our_keys) =
            Transaction::from_psbt(&our_root_fingerprint, &psbt, None).unwrap();
        assert_eq!(transaction, expected_transaction);
    }
}
