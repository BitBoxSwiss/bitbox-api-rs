use super::JavascriptError;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
type OnCloseCb = undefined | (() => void);
type Product = 'unknown' | 'bitbox02-multi' | 'bitbox02-btconly';
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
type BtcSignMessageSignature = {
  sig: Uint8Array,
  recid: bigint,
  electrumSig65: Uint8Array,
}
// nonce, gasPrice, gasLimit and value must be big-endian encoded, no trailing zeroes.
type EthTransaction = {
  nonce: Uint8Array;
  gasPrice: Uint8Array;
  gasLimit: Uint8Array;
  recipient: Uint8Array;
  value: Uint8Array;
  data: Uint8Array;
};
type EthSignature = {
  r: Uint8Array;
  s: Uint8Array;
  v: Uint8Array;
};
type CardanoXpub = Uint8Array;
type CardanoXpubs = CardanoXpub[];
type CardanoNetwork = 'mainnet' | 'testnet';
type CardanoScriptConfig = {
  pkhSkh: {
    keypathPayment: Keypath;
    keypathStake: Keypath;
  };
};
type CardanoInput = {
  keypath: Keypath;
  prevOutHash: Uint8Array;
  prevOutIndex: number;
};
type CardanoAssetGroupToken = {
  assetName: Uint8Array;
  value: bigint;
}
type CardanoAssetGroup = {
  policyId: Uint8Array;
  tokens: CardanoAssetGroupToken[];
}
type CardanoOutput = {
  encodedAddress: string;
  value: bigint;
  scriptConfig?: CardanoScriptConfig;
  assetGroups?: CardanoAssetGroup[];
}
type CardanoCertificate =
  | {
      stakeRegistration: {
        keypath: Keypath
      }
    }
  | {
      stakeDeregistration: {
        keypath: Keypath
      }
    }
  | {
      stakeDelegation: {
        keypath: Keypath
        poolKeyhash: Uint8Array
      }
    };
type CardanoWithdrawal = {
  keypath: Keypath;
  value: bigint;
}
type CardanoTransaction = {
  network: CardanoNetwork;
  inputs: CardanoInput[];
  outputs: CardanoOutput[];
  fee: bigint;
  ttl: bigint;
  certificates: CardanoCertificate[];
  withdrawals: CardanoWithdrawal[];
  validityIntervalStart: bigint;
  allowZeroTTL: boolean;
};
type CardanoShelleyWitness = {
  signature: Uint8Array;
  publicKey: Uint8Array;
}
type CardanoSignTransactionResult = {
  shelleyWitnesses: CardanoShelleyWitness[];
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
    #[wasm_bindgen(typescript_type = "OnCloseCb")]
    pub type TsOnCloseCb;
    #[wasm_bindgen(typescript_type = "Product")]
    pub type TsProduct;
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
    #[wasm_bindgen(typescript_type = "BtcSignMessageSignature")]
    pub type TsBtcSignMessageSignature;
    #[wasm_bindgen(typescript_type = "EthTransaction")]
    pub type TsEthTransaction;
    #[wasm_bindgen(typescript_type = "EthSignature")]
    pub type TsEthSignature;
    #[wasm_bindgen(typescript_type = "CardanoXpub")]
    pub type TsCardanoXpub;
    #[wasm_bindgen(typescript_type = "CardanoXpubs")]
    pub type TsCardanoXpubs;
    #[wasm_bindgen(typescript_type = "CardanoNetwork")]
    pub type TsCardanoNetwork;
    #[wasm_bindgen(typescript_type = "CardanoScriptConfig")]
    pub type TsCardanoScriptConfig;
    #[wasm_bindgen(typescript_type = "CardanoTransaction")]
    pub type TsCardanoTransaction;
    #[wasm_bindgen(typescript_type = "CardanoSignTransactionResult")]
    pub type TsCardanoSignTransactionResult;
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
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcScriptConfig"))
    }
}

impl TryFrom<TsBtcScriptConfigWithKeypath> for crate::pb::BtcScriptConfigWithKeypath {
    type Error = JavascriptError;
    fn try_from(value: TsBtcScriptConfigWithKeypath) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for BtcScriptConfigWithKeypath"))
    }
}

impl TryFrom<TsEthTransaction> for crate::eth::Transaction {
    type Error = JavascriptError;
    fn try_from(value: TsEthTransaction) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for EthTransaction"))
    }
}

impl TryFrom<TsCardanoNetwork> for crate::pb::CardanoNetwork {
    type Error = JavascriptError;
    fn try_from(value: TsCardanoNetwork) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for CardanoNetwork"))
    }
}

impl TryFrom<TsCardanoScriptConfig> for crate::pb::CardanoScriptConfig {
    type Error = JavascriptError;
    fn try_from(value: TsCardanoScriptConfig) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|_| JavascriptError::InvalidType("wrong type for CardanoScriptConfig"))
    }
}

impl TryFrom<TsCardanoTransaction> for crate::pb::CardanoSignTransactionRequest {
    type Error = JavascriptError;

    fn try_from(value: TsCardanoTransaction) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|e| JavascriptError::Foo(format!("wrong type for CardanoTransaction {:?}", e)))
    }
}

#[derive(serde::Serialize)]
pub struct EthSignature {
    pub r: Vec<u8>,
    pub s: Vec<u8>,
    pub v: Vec<u8>,
}
