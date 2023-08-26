//! Rust BitBox hardware wallet client library.

pub mod btc;
pub mod error;
mod noise;
pub mod runtime;
#[cfg(feature = "usb")]
pub mod usb;
#[cfg(feature = "wasm")]
pub mod wasm;

mod antiklepto;
mod communication;
mod constants;
mod keypath;
mod u2fframing;

/// BitBox protobuf messages.
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/shiftcrypto.bitbox02.rs"));
}

use crate::error::{BitBoxError, Error};

use pb::request::Request;
use pb::response::Response;
use runtime::Runtime;

use noise_protocol::DH;
use prost::Message;

use std::sync::{Arc, Mutex};

pub use keypath::Keypath;
#[cfg(feature = "serde")]
pub use noise::PersistedNoiseConfig;
pub use noise::{NoiseConfig, NoiseConfigNoCache};

use self::communication::HwwCommunication;

const FIRMWARE_CMD: u8 = 0x80 + 0x40 + 0x01;

const OP_I_CAN_HAS_HANDSHAEK: u8 = b'h';
const OP_HER_COMEZ_TEH_HANDSHAEK: u8 = b'H';
const OP_I_CAN_HAS_PAIRIN_VERIFICASHUN: u8 = b'v';
const OP_NOISE_MSG: u8 = b'n';
const _OP_ATTESTATION: u8 = b'a';
const OP_UNLOCK: u8 = b'u';

const RESPONSE_SUCCESS: u8 = 0x00;

type Cipher = noise_rust_crypto::ChaCha20Poly1305;
type HandshakeState =
    noise_protocol::HandshakeState<noise_rust_crypto::X25519, Cipher, noise_rust_crypto::Sha256>;

type CipherState = noise_protocol::CipherState<Cipher>;

pub struct BitBox<R: Runtime, T: communication::ReadWrite> {
    communication: communication::HwwCommunication<R, T>,
    noise_config: Box<dyn NoiseConfig>,
}

pub type PairingCode = String;

impl<R: Runtime, T: communication::ReadWrite> BitBox<R, T> {
    /// Creates a new BitBox instance. The provided noise config determines how the pairing
    /// information is persisted.
    ///
    /// Use `bitbox_api::PersistedNoiseConfig::new(...)` to persist the pairing in a JSON file
    /// (`serde` feature required) or provide your own implementation of the `NoiseConfig` trait.
    pub async fn from(
        device: T,
        noise_config: Box<dyn NoiseConfig>,
    ) -> Result<BitBox<R, T>, Error> {
        let u2f_communication = communication::U2fCommunication::from(device, FIRMWARE_CMD);
        Ok(BitBox {
            communication: HwwCommunication::from(u2f_communication).await?,
            noise_config,
        })
    }

    /// Invokes the device unlock and pairing.
    pub async fn unlock_and_pair(self) -> Result<PairingBitBox<R, T>, Error> {
        self.communication
            .query(&[OP_UNLOCK])
            .await
            .or(Err(Error::Unknown))?;
        self.pair().await
    }

    // fn validate_version(&self, comparison: &str) -> Result<(), ()> {
    //     if semver::VersionReq::parse(comparison)
    //         .or(Err(()))?
    //         .matches(&self.communication.info.version)
    //     {
    //         Ok(())
    //     } else {
    //         Err(())
    //     }
    // }

    async fn handshake_query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let mut framed_msg = vec![OP_HER_COMEZ_TEH_HANDSHAEK];
        framed_msg.extend_from_slice(msg);
        let mut response = self.communication.query(&framed_msg).await?;
        if response.is_empty() || response[0] != RESPONSE_SUCCESS {
            return Err(Error::Noise);
        }
        Ok(response.split_off(1))
    }

    async fn pair(self) -> Result<PairingBitBox<R, T>, Error> {
        let mut config_data = self.noise_config.read_config()?;
        let host_static_key = match config_data.get_app_static_privkey() {
            Some(k) => noise_rust_crypto::sensitive::Sensitive::from(k),
            None => {
                let k = noise_rust_crypto::X25519::genkey();
                config_data.set_app_static_privkey(&k[..])?;
                self.noise_config.store_config(&config_data)?;
                k
            }
        };
        let mut host = HandshakeState::new(
            noise_protocol::patterns::noise_xx(),
            true,
            b"Noise_XX_25519_ChaChaPoly_SHA256",
            Some(host_static_key),
            None,
            None,
            None,
        );

        if self
            .communication
            .query(&[OP_I_CAN_HAS_HANDSHAEK])
            .await?
            .as_slice()
            != [RESPONSE_SUCCESS]
        {
            return Err(Error::Noise);
        }

        let host_handshake_1 = host.write_message_vec(b"").or(Err(Error::Noise))?;
        let bb02_handshake_1 = self.handshake_query(&host_handshake_1).await?;

        host.read_message_vec(&bb02_handshake_1)
            .or(Err(Error::Noise))?;
        let host_handshake_2 = host.write_message_vec(b"").or(Err(Error::Noise))?;

        let bb02_handshake_2 = self.handshake_query(&host_handshake_2).await?;
        let remote_static_pubkey = host.get_rs().ok_or(Error::Noise)?;
        let pairing_verfication_required_by_app = !self
            .noise_config
            .read_config()?
            .contains_device_static_pubkey(&remote_static_pubkey);
        let pairing_verification_required_by_device = bb02_handshake_2.as_slice() == [0x01];
        if pairing_verfication_required_by_app || pairing_verification_required_by_device {
            let format_hash = |h| {
                let encoded = base32::encode(base32::Alphabet::RFC4648 { padding: true }, h);
                format!(
                    "{} {}\n{} {}",
                    &encoded[0..5],
                    &encoded[5..10],
                    &encoded[10..15],
                    &encoded[15..20]
                )
            };
            let handshake_hash: [u8; 32] = host.get_hash().try_into().or(Err(Error::Noise))?;
            let pairing_code = format_hash(&handshake_hash);

            Ok(PairingBitBox::from(
                self.communication,
                host,
                self.noise_config,
                Some(pairing_code),
            ))
        } else {
            Ok(PairingBitBox::from(
                self.communication,
                host,
                self.noise_config,
                None,
            ))
        }
    }
}

/// BitBox in the pairing state. Use `get_pairing_code()` to display the pairing code to the user
/// and `wait_confirm()` to proceed to the paired state.
pub struct PairingBitBox<R: Runtime, T: communication::ReadWrite> {
    communication: communication::HwwCommunication<R, T>,
    host: HandshakeState,
    noise_config: Box<dyn NoiseConfig>,
    pairing_code: Option<String>,
}

impl<R: Runtime, T: communication::ReadWrite> PairingBitBox<R, T> {
    fn from(
        communication: communication::HwwCommunication<R, T>,
        host: HandshakeState,
        noise_config: Box<dyn NoiseConfig>,
        pairing_code: Option<String>,
    ) -> Self {
        PairingBitBox {
            communication,
            host,
            noise_config,
            pairing_code,
        }
    }

    /// If a pairing code confirmation is required, this returns the pairing code. You must display
    /// it to the user and then call `wait_confirm()` to wait until the user confirms the code on
    /// the BitBox.
    ///
    /// If the BitBox was paired before and the pairing was peristed, the pairing step is
    /// skipped. In this case, `None` is returned. Also in this case, call `wait_confirm()` to
    /// establish the encrypted connection.
    pub fn get_pairing_code(&self) -> Option<String> {
        self.pairing_code.clone()
    }

    /// Proceed to the paired state.
    pub async fn wait_confirm(self) -> Result<PairedBitBox<R, T>, Error> {
        if self.pairing_code.is_some() {
            let response = self
                .communication
                .query(&[OP_I_CAN_HAS_PAIRIN_VERIFICASHUN])
                .await?;
            if response.as_slice() != [RESPONSE_SUCCESS] {
                return Err(Error::NoisePairingRejected);
            }

            let remote_static_pubkey = self.host.get_rs().ok_or(Error::Noise)?;
            let mut config_data = self.noise_config.read_config()?;
            config_data.add_device_static_pubkey(&remote_static_pubkey);
            self.noise_config.store_config(&config_data)?;
        }
        Ok(PairedBitBox::from(self.communication, self.host))
    }
}

/// Paired BitBox. This is where you can invoke most API functions like getting xpubs, displaying
/// receive addresses, etc.
pub struct PairedBitBox<R: Runtime, T: communication::ReadWrite> {
    communication: communication::HwwCommunication<R, T>,
    noise_send: Arc<Mutex<CipherState>>,
    noise_rcv: Arc<Mutex<CipherState>>,
}

impl<R: Runtime, T: communication::ReadWrite> PairedBitBox<R, T> {
    fn from(communication: communication::HwwCommunication<R, T>, host: HandshakeState) -> Self {
        let (send, recv) = host.get_ciphers();
        PairedBitBox {
            communication,
            noise_send: Arc::new(Mutex::new(send)),
            noise_rcv: Arc::new(Mutex::new(recv)),
        }
    }

    async fn query_proto(&self, request: Request) -> Result<Response, Error> {
        let msg = {
            let mut noise_send = self.noise_send.lock().unwrap();
            let mut encrypted = vec![OP_NOISE_MSG];
            let proto_msg = pb::Request {
                request: Some(request),
            };
            encrypted.extend_from_slice(&noise_send.encrypt_vec(&proto_msg.encode_to_vec()));
            encrypted
        };

        let response = self.communication.query(&msg).await?;
        if response.is_empty() || response[0] != RESPONSE_SUCCESS {
            return Err(Error::UnexpectedResponse);
        }
        let mut noise_rcv = self.noise_rcv.lock().unwrap();
        let decrypted = {
            noise_rcv
                .decrypt_vec(&response[1..])
                .or(Err(Error::Noise))?
        };
        match pb::Response::decode(&decrypted[..]) {
            Ok(pb::Response {
                response: Some(Response::Error(pb::Error { code, .. })),
            }) => match code {
                101 => Err(BitBoxError::InvalidInput.into()),
                102 => Err(BitBoxError::Memory.into()),
                103 => Err(BitBoxError::Generic.into()),
                104 => Err(BitBoxError::UserAbort.into()),
                105 => Err(BitBoxError::InvalidState.into()),
                106 => Err(BitBoxError::Disabled.into()),
                107 => Err(BitBoxError::Duplicate.into()),
                108 => Err(BitBoxError::NoiseEncrypt.into()),
                109 => Err(BitBoxError::NoiseDecrypt.into()),
                _ => Err(BitBoxError::Unknown.into()),
            },
            Ok(pb::Response {
                response: Some(response),
            }) => Ok(response),
            _ => Err(Error::ProtobufDecode),
        }
    }

    pub async fn device_info(&self) -> Result<pb::DeviceInfoResponse, Error> {
        match self
            .query_proto(Request::DeviceInfo(pb::DeviceInfoRequest {}))
            .await?
        {
            Response::DeviceInfo(di) => Ok(di),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Returns the hex-encoded 4-byte root fingerprint.
    pub async fn root_fingerprint(&self) -> Result<String, Error> {
        match self
            .query_proto(Request::Fingerprint(pb::RootFingerprintRequest {}))
            .await?
        {
            Response::Fingerprint(pb::RootFingerprintResponse { fingerprint }) => {
                Ok(hex::encode(fingerprint))
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Show recovery words on the Bitbox.
    pub async fn show_mnemonic(&self) -> Result<(), Error> {
        match self
            .query_proto(Request::ShowMnemonic(pb::ShowMnemonicRequest {}))
            .await?
        {
            Response::Success(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}
