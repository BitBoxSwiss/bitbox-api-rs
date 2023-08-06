use super::JavascriptError;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
type BtcCoin = 'btc' | 'tbtc' | 'ltc' | 'tltc';
type BtcFormatUnit = 'default' | 'sat';
type XPubType = 'tpub' | 'xpub' | 'ypub' | 'zpub' | 'vpub' | 'upub' | 'Vpub' | 'Zpub' | 'Upub' | 'Ypub';
type Keypath = string;
type DeviceInfo = {
  name: string;
  initialized: boolean;
  version: string;
  mnemonicPassphraseEnabled: boolean;
  securechipModel: string;
  monotonicIncrementsRemaining: number;
};
type SimpleType = 'p2wpkh-p2sh' | 'p2wpkh' | 'p2tr';
type BtcScriptConfig = { simpleType: SimpleType; };
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
    #[wasm_bindgen(typescript_type = "SimpleType")]
    pub type TsSimpleType;
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
        let js: JsValue = value.into();
        match js.as_string().as_deref() {
            Some("btc") => Ok(crate::pb::BtcCoin::Btc),
            Some("tbtc") => Ok(crate::pb::BtcCoin::Tbtc),
            Some("ltc") => Ok(crate::pb::BtcCoin::Ltc),
            Some("tltc") => Ok(crate::pb::BtcCoin::Tltc),
            _ => Err(JavascriptError::InvalidType("wrong type for BtcCoin")),
        }
    }
}

impl TryFrom<TsBtcFormatUnit> for crate::pb::btc_sign_init_request::FormatUnit {
    type Error = JavascriptError;
    fn try_from(value: TsBtcFormatUnit) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        match js.as_string().as_deref() {
            Some("default") => Ok(crate::pb::btc_sign_init_request::FormatUnit::Default),
            Some("sat") => Ok(crate::pb::btc_sign_init_request::FormatUnit::Sat),
            _ => Err(JavascriptError::InvalidType("wrong type for BtcFormatUnit")),
        }
    }
}

impl TryFrom<TsXPubType> for crate::pb::btc_pub_request::XPubType {
    type Error = JavascriptError;
    fn try_from(value: TsXPubType) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        match js.as_string().as_deref() {
            Some("tpub") => Ok(crate::pb::btc_pub_request::XPubType::Tpub),
            Some("xpub") => Ok(crate::pb::btc_pub_request::XPubType::Xpub),
            Some("ypub") => Ok(crate::pb::btc_pub_request::XPubType::Ypub),
            Some("zpub") => Ok(crate::pb::btc_pub_request::XPubType::Zpub),
            Some("vpub") => Ok(crate::pb::btc_pub_request::XPubType::Vpub),
            Some("upub") => Ok(crate::pb::btc_pub_request::XPubType::Upub),
            Some("Vpub") => Ok(crate::pb::btc_pub_request::XPubType::CapitalVpub),
            Some("Zpub") => Ok(crate::pb::btc_pub_request::XPubType::CapitalZpub),
            Some("Upub") => Ok(crate::pb::btc_pub_request::XPubType::CapitalUpub),
            Some("Ypub") => Ok(crate::pb::btc_pub_request::XPubType::CapitalYpub),
            _ => Err(JavascriptError::InvalidType("wrong type for XPubType")),
        }
    }
}

impl TryFrom<TsKeypath> for crate::Keypath {
    type Error = JavascriptError;
    fn try_from(value: TsKeypath) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        match js.as_string().as_deref() {
            Some(s) => Ok(s.try_into()?),
            None => Err(JavascriptError::InvalidType("wrong type for keypath")),
        }
    }
}

impl<'de> serde::Deserialize<'de> for crate::Keypath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.as_str().try_into().map_err(serde::de::Error::custom)
    }
}

impl TryFrom<TsSimpleType> for crate::pb::btc_script_config::SimpleType {
    type Error = JavascriptError;
    fn try_from(value: TsSimpleType) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        match js.as_string().as_deref() {
            Some("p2wpkh-p2sh") => Ok(crate::pb::btc_script_config::SimpleType::P2wpkhP2sh),
            Some("p2wpkh") => Ok(crate::pb::btc_script_config::SimpleType::P2wpkh),
            Some("p2tr") => Ok(crate::pb::btc_script_config::SimpleType::P2tr),
            _ => Err(JavascriptError::InvalidType("wrong type for SimpleType")),
        }
    }
}

impl TryFrom<TsBtcScriptConfig> for crate::pb::BtcScriptConfig {
    type Error = JavascriptError;
    fn try_from(value: TsBtcScriptConfig) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        match js_sys::Reflect::get(&js, &"simpleType".into()) {
            Ok(obj) => {
                let ts_simple_type: TsSimpleType = obj.into();
                let simple_type: crate::pb::btc_script_config::SimpleType =
                    ts_simple_type.try_into()?;
                Ok(crate::btc::make_script_config_simple(simple_type))
            }
            Err(_) => Err(JavascriptError::InvalidType(
                "wrong type for BtcScriptConfig",
            )),
        }
    }
}

impl TryFrom<TsBtcScriptConfigWithKeypath> for crate::pb::BtcScriptConfigWithKeypath {
    type Error = JavascriptError;
    fn try_from(value: TsBtcScriptConfigWithKeypath) -> Result<Self, Self::Error> {
        let js: JsValue = value.into();
        Ok(crate::pb::BtcScriptConfigWithKeypath {
            script_config: Some(match js_sys::Reflect::get(&js, &"scriptConfig".into()) {
                Ok(obj) => {
                    let ts_script_config: TsBtcScriptConfig = obj.into();
                    ts_script_config.try_into()?
                }
                Err(_) => {
                    return Err(JavascriptError::InvalidType(
                        "wrong type for BtcScriptConfigWithKeypath",
                    ))
                }
            }),
            keypath: match js_sys::Reflect::get(&js, &"keypath".into()) {
                Ok(obj) => {
                    let ts_keypath: TsKeypath = obj.into();
                    let keypath: crate::Keypath = ts_keypath.try_into()?;
                    keypath.to_vec()
                }
                Err(_) => {
                    return Err(JavascriptError::InvalidType(
                        "wrong type for BtcScriptConfigWithKeypath",
                    ))
                }
            },
        })
    }
}
