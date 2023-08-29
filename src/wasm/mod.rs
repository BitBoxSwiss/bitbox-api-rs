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
        "Could not open device. It might already have an open connection to this or another app."
    )]
    #[assoc(js_code = "could-not-open".into())]
    CouldNotOpen,
    #[error("connection aborted by user")]
    #[assoc(js_code="user-abort".into())]
    UserAbort,
    #[error("{0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = _0.js_code().into()))]
    BitBox(#[from] crate::error::Error),
    #[error("invalid JavaScript type: {0}")]
    #[assoc(js_code = "invalid-type".into())]
    InvalidType(&'static str),
    #[error("PSBT parse error: {0}")]
    #[assoc(js_code = "psbt-parse".into())]
    PsbtParseError(#[from] bitcoin::psbt::PsbtParseError),
}

impl From<JavascriptError> for JsValue {
    fn from(val: JavascriptError) -> Self {
        let obj = js_sys::Object::new();

        js_sys::Reflect::set(&obj, &"code".into(), &val.js_code().into()).unwrap();
        js_sys::Reflect::set(&obj, &"message".into(), &val.to_string().into()).unwrap();

        obj.into()
    }
}

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

#[wasm_bindgen]
pub struct BitBox(crate::BitBox<WasmRuntime>);

#[wasm_bindgen]
pub struct PairingBitBox(crate::PairingBitBox<WasmRuntime>);

#[wasm_bindgen]
pub struct PairedBitBox(crate::PairedBitBox<WasmRuntime>);

#[wasm_bindgen]
impl BitBox {
    #[wasm_bindgen(js_name = unlockAndPair)]
    pub async fn unlock_and_pair(self) -> Result<PairingBitBox, JavascriptError> {
        Ok(self.0.unlock_and_pair().await.map(PairingBitBox)?)
    }
}

#[wasm_bindgen]
impl PairingBitBox {
    #[wasm_bindgen(js_name = waitConfirm)]
    pub async fn wait_confirm(self) -> Result<PairedBitBox, JavascriptError> {
        Ok(self.0.wait_confirm().await.map(PairedBitBox)?)
    }

    #[wasm_bindgen(js_name = getPairingCode)]
    pub fn get_pairing_code(&self) -> Option<String> {
        self.0.get_pairing_code()
    }
}

#[wasm_bindgen]
impl PairedBitBox {
    #[wasm_bindgen(js_name = deviceInfo)]
    pub async fn device_info(&self) -> Result<types::TsDeviceInfo, JavascriptError> {
        let result = self.0.device_info().await?;
        Ok(serde_wasm_bindgen::to_value(&result).unwrap().into())
    }

    #[wasm_bindgen(js_name = rootFingerprint)]
    pub async fn root_fingerprint(&self) -> Result<String, JavascriptError> {
        Ok(self.0.root_fingerprint().await?)
    }

    #[wasm_bindgen(js_name = showMnemonic)]
    pub async fn show_mnemonic(&self) -> Result<(), JavascriptError> {
        Ok(self.0.show_mnemonic().await?)
    }

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
}
