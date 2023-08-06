use thiserror::Error;

#[cfg(feature = "wasm")]
use enum_assoc::Assoc;

#[cfg_attr(feature = "wasm", derive(Assoc), func(pub const fn js_code(&self) -> &'static str))]
#[derive(Error, Debug)]
pub enum BitBoxError {
    #[error("error code not recognized")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unknown"))]
    Unknown,
    #[error("invalid input")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "invalid-input"))]
    InvalidInput,
    #[error("memory")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "memory"))]
    Memory,
    #[error("generic error")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "generic"))]
    Generic,
    #[error("aborted by the user")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "user-abort"))]
    UserAbort,
    #[error("can't call this endpoint: wrong state")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "invalid-state"))]
    InvalidState,
    #[error("function disabled")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "disabled"))]
    Disabled,
    #[error("duplicate entry")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "duplicate"))]
    Duplicate,
    #[error("noise encryption failed")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "noise-encrypt"))]
    NoiseEncrypt,
    #[error("noise decryption failed")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "noise-decrypt"))]
    NoiseDecrypt,
}

#[cfg_attr(feature = "wasm", derive(Assoc), func(pub fn js_code(&self) -> String))]
#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown error")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unknown".into()))]
    Unknown,
    #[error("communication error: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "communication".into()))]
    Communication(#[from] crate::communication::Error),
    #[error("noise channel error")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "noise".into()))]
    Noise,
    #[error("noise config error: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "noise-config".into()))]
    NoiseConfig(#[from] crate::noise::ConfigError),
    #[error("pairing code rejected by user")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "pairing-rejected".into()))]
    NoisePairingRejected,
    #[error("BitBox returned an unexpected response")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "unexpected-response".into()))]
    UnexpectedResponse,
    #[error("protobuf message could not be decoded")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "protobuf-decode".into()))]
    ProtobufDecode,
    #[error("bitbox error: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = String::from("bitbox-") + _0.js_code().into()))]
    BitBox(#[from] BitBoxError),
    #[error("failed parsing keypath: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "keypath-parse".into()))]
    KeypathParse(String),
    #[error("PSBT error: {0}")]
    #[cfg_attr(feature = "wasm", assoc(js_code = String::from("psbt-") + _0.js_code().into()))]
    Psbt(#[from] crate::btc::PsbtError),
    #[error("Unexpected signature format returned by BitBox")]
    #[cfg_attr(feature = "wasm", assoc(js_code = "keypath-parse".into()))]
    InvalidSignature,
}
