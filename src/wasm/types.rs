use super::JavascriptError;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
type BtcCoin = 'btc' | 'tbtc' | 'ltc' | 'tltc';
type BtcFormatUnit = 'default' | 'sat';
type XPubType = 'tpub' | 'xpub' | 'ypub' | 'zpub' | 'vpub' | 'upub' | 'Vpub' | 'Zpub' | 'Upub' | 'Ypub';
type Keypath = string;
type XPub = string;
type DeviceInfo = {
  name: string;
  initialized: boolean;
  version: string;
  mnemonicPassphraseEnabled: boolean;
  securechipModel: string;
  monotonicIncrementsRemaining: number;
};
type BtcSimpleType = 'p2wpkhP2sh' | 'p2wpkh' | 'p2tr';
type KeyOriginInfo = {
  rootFingerprint?: string;
  keypath?: Keypath;
  xpub: XPub;
};
type BtcRegisterXPubType = 'autoElectrum' | 'autoXpubTpub';
type BtcPolicy = { policy: string; keys: KeyOriginInfo[] };
type BtcScriptConfig = { simpleType: BtcSimpleType; } | { policy: BtcPolicy };
type BtcScriptConfigWithKeypath = {
  scriptConfig: BtcScriptConfig;
  keypath: Keypath;
};
type Error = {
  code: string;
  message: string;
  // original JS error if code === 'unknown-js'
  err?: any;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "BtcCoin")]
    pub type TsBtcCoin;
    #[wasm_bindgen(typescript_type = "BtcFormatUnit")]
    pub type TsBtcFormatUnit;
    #[wasm_bindgen(typescript_type = "XPubType")]
    pub type TsXPubType;
    #[wasm_bindgen(typescript_type = "Keypath")]
    pub type TsKeypath;
    #[wasm_bindgen(typescript_type = "DeviceInfo")]
    pub type TsDeviceInfo;
    #[wasm_bindgen(typescript_type = "BtcRegisterXPubType")]
    pub type TsBtcRegisterXPubType;
    #[wasm_bindgen(typescript_type = "BtcSimpleType")]
    pub type TsBtcSimpleType;
    #[wasm_bindgen(typescript_type = "KeyOriginInfo")]
    pub type TsKeyOriginInfo;
    #[wasm_bindgen(typescript_type = "BtcPolicy")]
    pub type TsBtcPolicy;
    #[wasm_bindgen(typescript_type = "BtcScriptConfig")]
    pub type TsBtcScriptConfig;
    #[wasm_bindgen(typescript_type = "BtcScriptConfigWithKeypath")]
    pub type TsBtcScriptConfigWithKeypath;
    #[wasm_bindgen(typescript_type = "Error")]
    pub type TsError;
}

impl TryFrom<TsBtcCoin> for crate::pb::BtcCoin {
    type Error = JavascriptError;
    fn try_from(value: TsBtcCoin) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcCoin"))
    }
}

impl TryFrom<TsBtcFormatUnit> for crate::pb::btc_sign_init_request::FormatUnit {
    type Error = JavascriptError;
    fn try_from(value: TsBtcFormatUnit) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcFormatUnit"))
    }
}

impl TryFrom<TsXPubType> for crate::pb::btc_pub_request::XPubType {
    type Error = JavascriptError;
    fn try_from(value: TsXPubType) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for XPubType"))
    }
}

impl TryFrom<TsKeypath> for crate::Keypath {
    type Error = JavascriptError;
    fn try_from(value: TsKeypath) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for Keypath"))
    }
}

impl TryFrom<TsBtcRegisterXPubType> for crate::pb::btc_register_script_config_request::XPubType {
    type Error = JavascriptError;
    fn try_from(value: TsBtcRegisterXPubType) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcRegisterXPubType"))
    }
}

impl TryFrom<TsBtcPolicy> for crate::pb::btc_script_config::Policy {
    type Error = JavascriptError;

    fn try_from(value: TsBtcPolicy) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcPolicy"))
    }
}

impl TryFrom<TsBtcScriptConfig> for crate::pb::BtcScriptConfig {
    type Error = JavascriptError;
    fn try_from(value: TsBtcScriptConfig) -> Result<Self, Self::Error> {
        let config: crate::pb::btc_script_config::Config =
            serde_wasm_bindgen::from_value(value.into())
                .map_err(|_| JavascriptError::InvalidType("wrong type for BtcScriptConfig"))?;
        Ok(crate::pb::BtcScriptConfig {
            config: Some(config),
        })
    }
}

impl TryFrom<TsBtcScriptConfigWithKeypath> for crate::pb::BtcScriptConfigWithKeypath {
    type Error = JavascriptError;
    fn try_from(value: TsBtcScriptConfigWithKeypath) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcScriptConfigWithKeypath"))
    }
}
