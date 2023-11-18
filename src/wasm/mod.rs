use std::str::FromStr;

mod connect;
mod localstorage;
mod noise;
mod types;

use wasm_bindgen::prelude::*;

use thiserror::Error;

use enum_assoc::Assoc;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Assoc)]
#[func(pub fn js_code(&self) -> String)]
#[derive(Error, Debug)]
pub enum JavascriptError {
    #[error("Unknown Javascript error")]
    #[assoc(js_code = "unknown-js".into())]
    Unknown,
    #[error(
        "Could not open device. It might already have an open connection to another app. If so, please close the other app first."
    )]
    #[assoc(js_code = "could-not-open".into())]
    CouldNotOpenWebHID,
    #[error("Could not open device. {0}")]
    #[assoc(js_code = "could-not-open".into())]
    CouldNotOpenBridge(String),
    #[error("connection aborted by user")]
    #[assoc(js_code="user-abort".into())]
    UserAbort,
    #[error("{0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = _0.js_code().into()))]
    BitBox(#[from] crate::error::Error),
    #[error("invalid JavaScript type: {0}")]
    #[assoc(js_code = "invalid-type".into())]
    InvalidType(&'static str),
    #[error("invalid JavaScript type: {0}")]
    #[assoc(js_code = "invalid-type".into())]
    Foo(String),
    #[error("PSBT parse error: {0}")]
    #[assoc(js_code = "psbt-parse".into())]
    PsbtParseError(#[from] bitcoin::psbt::PsbtParseError),
    #[error("Chain ID too large and would overflow in the computation of the `v` signature value: {chain_id}")]
    #[assoc(js_code = "chain-id-too-large".into())]
    ChainIDTooLarge { chain_id: u64 },
}

impl From<JavascriptError> for JsValue {
    fn from(val: JavascriptError) -> Self {
        let obj = js_sys::Object::new();

        js_sys::Reflect::set(&obj, &"code".into(), &val.js_code().into()).unwrap();
        js_sys::Reflect::set(&obj, &"message".into(), &val.to_string().into()).unwrap();

        obj.into()
    }
}

/// Run any exception raised by this library through this function to get a typed error.
///
/// Example:
/// ```JavaScript
/// try { ... }
/// catch (err) {
///   const typedErr: Error = bitbox.ensureError(err);
///   // Handle error by checking the error code, displaying the error message, etc.
/// }
///
/// See also: isUserAbort().
#[wasm_bindgen(js_name = ensureError)]
pub fn ensure_error(err: JsValue) -> types::TsError {
    let code = js_sys::Reflect::get(&err, &"code".into());
    let message = js_sys::Reflect::get(&err, &"message".into());
    match (code, message) {
        (Ok(code), Ok(message)) if code.is_string() && message.is_string() => err.into(),
        _ => {
            let js_result: JsValue = JavascriptError::Unknown.into();
            js_sys::Reflect::set(&js_result, &"err".into(), &err).unwrap();
            js_result.into()
        }
    }
}

/// Returns true if the user cancelled an operation.
#[wasm_bindgen(js_name = isUserAbort)]
pub fn is_user_abort(err: types::TsError) -> bool {
    match js_sys::Reflect::get(&err, &"code".into()) {
        Ok(code) => matches!(
            code.as_string().as_deref(),
            Some("user-abort" | "bitbox-user-abort")
        ),
        _ => false,
    }
}

#[wasm_bindgen(raw_module = "./webhid")]
extern "C" {
    async fn jsSleep(millis: f64);
}

struct WasmRuntime;

#[async_trait::async_trait(?Send)]
impl crate::runtime::Runtime for WasmRuntime {
    async fn sleep(dur: std::time::Duration) {
        jsSleep(dur.as_millis() as _).await
    }
}

/// BitBox client. Instantiate it using `bitbox02ConnectAuto()`.
#[wasm_bindgen]
pub struct BitBox(crate::BitBox<WasmRuntime>);

/// BitBox in the pairing state. Use `getPairingCode()` to display the pairing code to the user and
/// `waitConfirm()` to proceed to the paired state.
#[wasm_bindgen]
pub struct PairingBitBox(crate::PairingBitBox<WasmRuntime>);

/// Paired BitBox. This is where you can invoke most API functions like getting xpubs, displaying
/// receive addresses, etc.
#[wasm_bindgen]
pub struct PairedBitBox(crate::PairedBitBox<WasmRuntime>);

#[wasm_bindgen]
impl BitBox {
    /// Invokes the device unlock and pairing. After this, stop using this instance and continue
    /// with the returned instance of type `PairingBitBox`.
    #[wasm_bindgen(js_name = unlockAndPair)]
    pub async fn unlock_and_pair(self) -> Result<PairingBitBox, JavascriptError> {
        Ok(self.0.unlock_and_pair().await.map(PairingBitBox)?)
    }
}

/// BitBox in the pairing state. Use `getPairingCode()` to display the pairing code to the user and
/// `waitConfirm()` to proceed to the paired state.
#[wasm_bindgen]
impl PairingBitBox {
    /// If a pairing code confirmation is required, this returns the pairing code. You must display
    /// it to the user and then call `waitConfirm()` to wait until the user confirms the code on
    /// the BitBox.
    ///
    /// If the BitBox was paired before and the pairing was persisted, the pairing step is
    /// skipped. In this case, `undefined` is returned. Also in this case, call `waitConfirm()` to
    /// establish the encrypted connection.
    #[wasm_bindgen(js_name = getPairingCode)]
    pub fn get_pairing_code(&self) -> Option<String> {
        self.0.get_pairing_code()
    }

    /// Proceed to the paired state. After this, stop using this instance and continue with the
    /// returned instance of type `PairedBitBox`.
    #[wasm_bindgen(js_name = waitConfirm)]
    pub async fn wait_confirm(self) -> Result<PairedBitBox, JavascriptError> {
        Ok(self.0.wait_confirm().await.map(PairedBitBox)?)
    }
}

fn compute_v(chain_id: u64, rec_id: u8) -> Option<u64> {
    let v_offset: u64 = chain_id.checked_mul(2)?.checked_add(8)?;
    (rec_id as u64 + 27).checked_add(v_offset)
}

/// Paired BitBox. This is where you can invoke most API functions like getting xpubs, displaying
/// receive addresses, etc.
#[wasm_bindgen]
impl PairedBitBox {
    #[wasm_bindgen(js_name = deviceInfo)]
    pub async fn device_info(&self) -> Result<types::TsDeviceInfo, JavascriptError> {
        let result = self.0.device_info().await?;
        Ok(serde_wasm_bindgen::to_value(&result).unwrap().into())
    }

    /// Returns which product we are connected to.
    #[wasm_bindgen(js_name = product)]
    pub fn product(&self) -> types::TsProduct {
        match self.0.product() {
            crate::Product::Unknown => JsValue::from_str("unknown").into(),
            crate::Product::BitBox02Multi => JsValue::from_str("bitbox02-multi").into(),
            crate::Product::BitBox02BtcOnly => JsValue::from_str("bitbox02-btconly").into(),
        }
    }

    /// Returns the hex-encoded 4-byte root fingerprint.
    #[wasm_bindgen(js_name = rootFingerprint)]
    pub async fn root_fingerprint(&self) -> Result<String, JavascriptError> {
        Ok(self.0.root_fingerprint().await?)
    }

    /// Show recovery words on the Bitbox.
    #[wasm_bindgen(js_name = showMnemonic)]
    pub async fn show_mnemonic(&self) -> Result<(), JavascriptError> {
        Ok(self.0.show_mnemonic().await?)
    }

    /// Retrieves an xpub. For non-standard keypaths, a warning is displayed on the BitBox even if
    /// `display` is false.
    #[wasm_bindgen(js_name = btcXpub)]
    pub async fn btc_xpub(
        &self,
        coin: types::TsBtcCoin,
        keypath: types::TsKeypath,
        xpub_type: types::TsXPubType,
        display: bool,
    ) -> Result<String, JavascriptError> {
        Ok(self
            .0
            .btc_xpub(
                coin.try_into()?,
                &keypath.try_into()?,
                xpub_type.try_into()?,
                display,
            )
            .await?)
    }

    /// Before a multisig or policy script config can be used to display receive addresses or sign
    /// transactions, it must be registered on the device. This function checks if the script config
    /// was already registered.
    ///
    /// `keypath_account` must be set if the script config is multisig, and can be `undefined` if it
    /// is a policy.
    #[wasm_bindgen(js_name = btcIsScriptConfigRegistered)]
    pub async fn btc_is_script_config_registered(
        &self,
        coin: types::TsBtcCoin,
        script_config: types::TsBtcScriptConfig,
        keypath_account: Option<types::TsKeypath>,
    ) -> Result<bool, JavascriptError> {
        Ok(self
            .0
            .btc_is_script_config_registered(
                coin.try_into()?,
                &script_config.try_into()?,
                keypath_account
                    .map(|kp| kp.try_into())
                    .transpose()?
                    .as_ref(),
            )
            .await?)
    }

    /// Before a multisig or policy script config can be used to display receive addresses or sign
    /// transcations, it must be registered on the device.
    ///
    /// If no name is provided, the user will be asked to enter it on the device instead.  If
    /// provided, it must be non-empty, smaller or equal to 30 chars, consist only of printable
    /// ASCII characters, and contain no whitespace other than spaces.
    ///
    ///
    /// `keypath_account` must be set if the script config is multisig, and can be `undefined` if it
    /// is a policy.
    #[wasm_bindgen(js_name = btcRegisterScriptConfig)]
    pub async fn btc_register_script_config(
        &self,
        coin: types::TsBtcCoin,
        script_config: types::TsBtcScriptConfig,
        keypath_account: Option<types::TsKeypath>,
        xpub_type: types::TsBtcRegisterXPubType,
        name: Option<String>,
    ) -> Result<(), JavascriptError> {
        Ok(self
            .0
            .btc_register_script_config(
                coin.try_into()?,
                &script_config.try_into()?,
                keypath_account
                    .map(|kp| kp.try_into())
                    .transpose()?
                    .as_ref(),
                xpub_type.try_into()?,
                name.as_deref(),
            )
            .await?)
    }

    /// Retrieves a Bitcoin address at the provided keypath.
    ///
    /// For the simple script configs (single-sig), the keypath must follow the
    /// BIP44/BIP49/BIP84/BIP86 conventions.
    #[wasm_bindgen(js_name = btcAddress)]
    pub async fn btc_address(
        &self,
        coin: types::TsBtcCoin,
        keypath: types::TsKeypath,
        script_config: types::TsBtcScriptConfig,
        display: bool,
    ) -> Result<String, JavascriptError> {
        Ok(self
            .0
            .btc_address(
                coin.try_into()?,
                &keypath.try_into()?,
                &script_config.try_into()?,
                display,
            )
            .await?)
    }

    /// Sign a PSBT.
    ///
    /// If `force_script_config` is `undefined`, we attempt to infer the involved script
    /// configs. For the simple script config (single sig), we infer the script config from the
    /// involved redeem scripts and provided derviation paths.
    ///
    /// Multisig and policy configs are currently not inferred and must be provided using
    /// `force_script_config`.
    #[wasm_bindgen(js_name = btcSignPSBT)]
    pub async fn btc_sign_psbt(
        &self,
        coin: types::TsBtcCoin,
        psbt: &str,
        force_script_config: Option<types::TsBtcScriptConfigWithKeypath>,
        format_unit: types::TsBtcFormatUnit,
    ) -> Result<String, JavascriptError> {
        let mut psbt = bitcoin::psbt::Psbt::from_str(psbt.trim())?;
        self.0
            .btc_sign_psbt(
                coin.try_into()?,
                &mut psbt,
                match force_script_config {
                    Some(sc) => Some(sc.try_into()?),
                    None => None,
                },
                format_unit.try_into()?,
            )
            .await?;
        Ok(psbt.to_string())
    }

    #[wasm_bindgen(js_name = btcSignMessage)]
    pub async fn btc_sign_message(
        &self,
        coin: types::TsBtcCoin,
        script_config: types::TsBtcScriptConfigWithKeypath,
        msg: &[u8],
    ) -> Result<types::TsBtcSignMessageSignature, JavascriptError> {
        let signature = self
            .0
            .btc_sign_message(coin.try_into()?, script_config.try_into()?, msg)
            .await?;

        Ok(serde_wasm_bindgen::to_value(&signature).unwrap().into())
    }

    /// Does this device support ETH functionality? Currently this means BitBox02 Multi.
    #[wasm_bindgen(js_name = ethSupported)]
    pub fn eth_supported(&self) -> bool {
        self.0.eth_supported()
    }

    /// Query the device for an xpub.
    #[wasm_bindgen(js_name = ethXpub)]
    pub async fn eth_xpub(&self, keypath: types::TsKeypath) -> Result<String, JavascriptError> {
        Ok(self.0.eth_xpub(&keypath.try_into()?).await?)
    }

    /// Query the device for an Ethereum address.
    #[wasm_bindgen(js_name = ethAddress)]
    pub async fn eth_address(
        &self,
        chain_id: u64,
        keypath: types::TsKeypath,
        display: bool,
    ) -> Result<String, JavascriptError> {
        Ok(self
            .0
            .eth_address(chain_id, &keypath.try_into()?, display)
            .await?)
    }

    /// Signs an Ethereum transaction. It returns a 65 byte signature (R, S, and 1 byte recID).
    #[wasm_bindgen(js_name = ethSignTransaction)]
    pub async fn eth_sign_transaction(
        &self,
        chain_id: u64,
        keypath: types::TsKeypath,
        tx: types::TsEthTransaction,
    ) -> Result<types::TsEthSignature, JavascriptError> {
        let signature = self
            .0
            .eth_sign_transaction(chain_id, &keypath.try_into()?, &tx.try_into()?)
            .await?;

        let v: u64 = compute_v(chain_id, signature[64])
            .ok_or(JavascriptError::ChainIDTooLarge { chain_id })?;
        Ok(serde_wasm_bindgen::to_value(&types::EthSignature {
            r: signature[..32].to_vec(),
            s: signature[32..64].to_vec(),
            v: crate::util::remove_leading_zeroes(&v.to_be_bytes()),
        })
        .unwrap()
        .into())
    }

    /// Signs an Ethereum message. The provided msg will be prefixed with "\x19Ethereum message\n" +
    /// len(msg) in the hardware, e.g. "\x19Ethereum\n5hello" (yes, the len prefix is the ascii
    /// representation with no fixed size or delimiter).  It returns a 65 byte signature (R, S, and
    /// 1 byte recID). 27 is added to the recID to denote an uncompressed pubkey.
    #[wasm_bindgen(js_name = ethSignMessage)]
    pub async fn eth_sign_message(
        &self,
        chain_id: u64,
        keypath: types::TsKeypath,
        msg: &[u8],
    ) -> Result<types::TsEthSignature, JavascriptError> {
        let signature = self
            .0
            .eth_sign_message(chain_id, &keypath.try_into()?, msg)
            .await?;

        Ok(serde_wasm_bindgen::to_value(&types::EthSignature {
            r: signature[..32].to_vec(),
            s: signature[32..64].to_vec(),
            v: vec![signature[64]], // offset of 27 is already included
        })
        .unwrap()
        .into())
    }

    /// Signs an Ethereum EIP-712 typed message. It returns a 65 byte signature (R, S, and 1 byte
    /// recID). 27 is added to the recID to denote an uncompressed pubkey.
    #[wasm_bindgen(js_name = ethSignTypedMessage)]
    pub async fn eth_sign_typed_message(
        &self,
        chain_id: u64,
        keypath: types::TsKeypath,
        msg: JsValue,
    ) -> Result<types::TsEthSignature, JavascriptError> {
        let json_msg: String = js_sys::JSON::stringify(&msg).unwrap().into();
        let signature = self
            .0
            .eth_sign_typed_message(chain_id, &keypath.try_into()?, &json_msg)
            .await?;

        Ok(serde_wasm_bindgen::to_value(&types::EthSignature {
            r: signature[..32].to_vec(),
            s: signature[32..64].to_vec(),
            v: vec![signature[64]], // offset of 27 is already included
        })
        .unwrap()
        .into())
    }

    /// Does this device support Cardano functionality? Currently this means BitBox02 Multi.
    #[wasm_bindgen(js_name = cardanoSupported)]
    pub fn cardano_supported(&self) -> bool {
        self.0.cardano_supported()
    }

    /// Query the device for xpubs. The result contains one xpub per requested keypath. Each xpub is
    /// 64 bytes: 32 byte chain code + 32 byte pubkey.
    #[wasm_bindgen(js_name = cardanoXpubs)]
    pub async fn cardano_xpubs(
        &self,
        keypaths: Vec<types::TsKeypath>,
    ) -> Result<types::TsCardanoXpubs, JavascriptError> {
        let xpubs = self
            .0
            .cardano_xpubs(
                keypaths
                    .into_iter()
                    .map(|kp| kp.try_into())
                    .collect::<Result<Vec<crate::Keypath>, _>>()?
                    .as_slice(),
            )
            .await?;
        Ok(serde_wasm_bindgen::to_value(&xpubs).unwrap().into())
    }

    /// Query the device for a Cardano address.
    #[wasm_bindgen(js_name = cardanoAddress)]
    pub async fn cardano_address(
        &self,
        network: types::TsCardanoNetwork,
        script_config: types::TsCardanoScriptConfig,
        display: bool,
    ) -> Result<String, JavascriptError> {
        Ok(self
            .0
            .cardano_address(network.try_into()?, &script_config.try_into()?, display)
            .await?)
    }

    /// Sign a Cardano transaction.
    #[wasm_bindgen(js_name = cardanoSignTransaction)]
    pub async fn cardano_sign_transaction(
        &self,
        transaction: types::TsCardanoTransaction,
    ) -> Result<types::TsCardanoSignTransactionResult, JavascriptError> {
        let tt = transaction.try_into()?;
        let result = self.0.cardano_sign_transaction(tt).await?;
        Ok(serde_wasm_bindgen::to_value(&result).unwrap().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_v() {
        // Test with some known values
        assert_eq!(compute_v(1, 0), Some(37));
        assert_eq!(compute_v(1, 1), Some(38));

        // Test with a chain_id that would cause overflow when multiplied by 2
        let large_chain_id = u64::MAX / 2 + 1;
        assert_eq!(compute_v(large_chain_id, 0), None);

        // Test with values that would cause overflow in the final addition
        let chain_id_close_to_overflow = (u64::MAX - 35) / 2;
        assert_eq!(compute_v(chain_id_close_to_overflow, 1), None);
    }
}
