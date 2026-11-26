// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

use crate::communication;

#[derive(Error, Debug)]
pub enum BitBoxError {
    #[error("error code not recognized")]
    Unknown,
    #[error("invalid input")]
    InvalidInput,
    #[error("memory")]
    Memory,
    #[error("generic error")]
    Generic,
    #[error("aborted by the user")]
    UserAbort,
    #[error("can't call this endpoint: wrong state")]
    InvalidState,
    #[error("function disabled")]
    Disabled,
    #[error("duplicate entry")]
    Duplicate,
    #[error("noise encryption failed")]
    NoiseEncrypt,
    #[error("noise decryption failed")]
    NoiseDecrypt,
}

impl BitBoxError {
    pub fn from_code(code: i32) -> Self {
        match code {
            101 => Self::InvalidInput,
            102 => Self::Memory,
            103 => Self::Generic,
            104 => Self::UserAbort,
            105 => Self::InvalidState,
            106 => Self::Disabled,
            107 => Self::Duplicate,
            108 => Self::NoiseEncrypt,
            109 => Self::NoiseDecrypt,
            _ => Self::Unknown,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown error")]
    Unknown,
    #[error("firmware version {0} required")]
    Version(&'static str),
    #[cfg(feature = "usb")]
    #[error("hid error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[cfg(feature = "simulator")]
    #[error("simulator error: {0}")]
    Simulator(#[from] crate::simulator::Error),
    #[error("communication error: {0}")]
    Communication(communication::Error),
    #[error("noise channel error")]
    Noise,
    #[cfg(any(feature = "client", feature = "persistence"))]
    #[error("noise config error: {0}")]
    NoiseConfig(#[from] crate::noise::ConfigError),
    #[cfg(feature = "client")]
    #[error("pairing code rejected by user")]
    NoisePairingRejected,
    #[error("BitBox returned an unexpected response")]
    UnexpectedResponse,
    #[error("protobuf message could not be decoded")]
    ProtobufDecode,
    #[error("bitbox error: {0}")]
    BitBox(#[from] BitBoxError),
    #[error("failed parsing keypath: {0}")]
    KeypathParse(String),
    #[error("PSBT error: {0}")]
    Psbt(#[from] crate::btc::PsbtError),
    #[error("Unexpected signature format returned by BitBox")]
    InvalidSignature,
    #[error("Antiklepto verification failed: {0}")]
    AntiKlepto(#[from] crate::antiklepto::Error),
    #[cfg(all(feature = "client", feature = "persistence"))]
    #[error("EIP-712 typed message processing error: {0}")]
    EthTypedMessage(String),
    #[error("Bitcoin transaction signing error: {0}")]
    BtcSign(String),
}

impl From<communication::Error> for Error {
    fn from(value: communication::Error) -> Self {
        match value {
            communication::Error::Version(s) => Error::Version(s),
            e => Error::Communication(e),
        }
    }
}
