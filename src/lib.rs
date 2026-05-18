// SPDX-License-Identifier: Apache-2.0

//! Rust BitBox hardware wallet client library.

#[cfg(all(feature = "wasm", feature = "multithreaded"))]
compile_error!("wasm and multithreaded can't both be active");

pub mod btc;
pub mod cardano;
pub mod error;
pub mod eth;
mod noise;
pub mod runtime;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "usb")]
pub mod usb;
#[cfg(feature = "wasm")]
pub mod wasm;

mod antiklepto;
mod communication;
mod constants;
mod keypath;
mod u2fframing;
mod util;

/// BitBox protobuf messages.
#[allow(clippy::all)]
pub mod pb {
    include!("./shiftcrypto.bitbox02.rs");
}

use crate::error::{BitBoxError, Error};

use pb::request::Request;
use pb::response::Response;
use runtime::Runtime;

use noise_protocol::DH;
use prost::Message;

use futures_util::lock::{Mutex as AsyncMutex, MutexGuard as ApiMutexGuard};
use std::sync::Mutex;

pub use keypath::Keypath;
pub use noise::PersistedNoiseConfig;
pub use noise::{ConfigError, NoiseConfig, NoiseConfigData, NoiseConfigNoCache};
pub use util::Threading;

use communication::HwwCommunication;

pub use communication::Product;

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

/// BitBox client. See `from_hid_device()`.
pub struct BitBox<R: Runtime> {
    communication: communication::HwwCommunication<R>,
    noise_config: Box<dyn NoiseConfig>,
}

pub type PairingCode = String;

impl<R: Runtime> BitBox<R> {
    async fn from(
        device: Box<dyn communication::ReadWrite>,
        noise_config: Box<dyn NoiseConfig>,
    ) -> Result<BitBox<R>, Error> {
        Ok(BitBox {
            communication: HwwCommunication::from(device).await?,
            noise_config,
        })
    }

    /// Creates a new BitBox instance. The provided noise config determines how the pairing
    /// information is persisted. Use `usb::get_any_bitbox02()` to find a BitBox02 HID device.
    ///
    /// Use `bitbox_api::PersistedNoiseConfig::new(...)` to persist the pairing in a JSON file
    /// (`serde` feature required) or provide your own implementation of the `NoiseConfig` trait.
    #[cfg(feature = "usb")]
    pub async fn from_hid_device(
        device: hidapi::HidDevice,
        noise_config: Box<dyn NoiseConfig>,
    ) -> Result<BitBox<R>, Error> {
        let comm = Box::new(communication::U2fHidCommunication::from(
            Box::new(crate::usb::HidDevice::new(device)),
            communication::FIRMWARE_CMD,
        ));
        Self::from(comm, noise_config).await
    }

    #[cfg(feature = "simulator")]
    pub async fn from_simulator(
        endpoint: Option<&str>,
        noise_config: Box<dyn NoiseConfig>,
    ) -> Result<BitBox<R>, Error> {
        let comm = Box::new(communication::U2fHidCommunication::from(
            crate::simulator::try_connect::<R>(endpoint).await?,
            communication::FIRMWARE_CMD,
        ));
        Self::from(comm, noise_config).await
    }

    /// Invokes the device unlock and pairing.
    pub async fn unlock_and_pair(self) -> Result<PairingBitBox<R>, Error> {
        self.communication
            .query(&[OP_UNLOCK])
            .await
            .or(Err(Error::Unknown))?;
        self.pair().await
    }

    async fn handshake_query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let mut framed_msg = vec![OP_HER_COMEZ_TEH_HANDSHAEK];
        framed_msg.extend_from_slice(msg);
        let mut response = self.communication.query(&framed_msg).await?;
        if response.is_empty() || response[0] != RESPONSE_SUCCESS {
            return Err(Error::Noise);
        }
        Ok(response.split_off(1))
    }

    async fn pair(self) -> Result<PairingBitBox<R>, Error> {
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
pub struct PairingBitBox<R: Runtime> {
    communication: communication::HwwCommunication<R>,
    host: HandshakeState,
    noise_config: Box<dyn NoiseConfig>,
    pairing_code: Option<String>,
}

impl<R: Runtime> PairingBitBox<R> {
    fn from(
        communication: communication::HwwCommunication<R>,
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
    /// If the BitBox was paired before and the pairing was persisted, the pairing step is
    /// skipped. In this case, `None` is returned. Also in this case, call `wait_confirm()` to
    /// establish the encrypted connection.
    pub fn get_pairing_code(&self) -> Option<String> {
        self.pairing_code.clone()
    }

    /// Proceed to the paired state.
    pub async fn wait_confirm(self) -> Result<PairedBitBox<R>, Error> {
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
pub struct PairedBitBox<R: Runtime> {
    communication: communication::HwwCommunication<R>,
    api_mutex: AsyncMutex<()>,
    noise_send: Mutex<CipherState>,
    noise_recv: Mutex<CipherState>,
}

impl<R: Runtime> PairedBitBox<R> {
    fn from(communication: communication::HwwCommunication<R>, host: HandshakeState) -> Self {
        let (send, recv) = host.get_ciphers();
        PairedBitBox {
            communication,
            api_mutex: AsyncMutex::new(()),
            noise_send: Mutex::new(send),
            noise_recv: Mutex::new(recv),
        }
    }

    /// Serializes all public API calls that touch the device.
    ///
    /// The BitBox protocol is an ordered request/response conversation, not a
    /// multiplexed transport. Some public calls span multiple encrypted
    /// requests, so this mutex guard is held for the whole public method
    /// instead of only one query_proto() round trip.
    pub(crate) async fn begin_api_call(&self) -> ApiMutexGuard<'_, ()> {
        self.api_mutex.lock().await
    }

    fn validate_version(&self, comparison: &'static str) -> Result<(), Error> {
        if semver::VersionReq::parse(comparison)
            .or(Err(Error::Unknown))?
            .matches(&self.communication.info.version)
        {
            Ok(())
        } else {
            Err(Error::Version(comparison))
        }
    }

    async fn query_proto(&self, request: Request) -> Result<Response, Error> {
        let mut encrypted = vec![OP_NOISE_MSG];
        encrypted.extend_from_slice({
            let mut send = self.noise_send.lock().unwrap();
            let proto_msg = pb::Request {
                request: Some(request),
            };
            &send.encrypt_vec(&proto_msg.encode_to_vec())
        });

        let response = self.communication.query(&encrypted).await?;
        if response.is_empty() || response[0] != RESPONSE_SUCCESS {
            return Err(Error::UnexpectedResponse);
        }
        let decrypted = {
            let mut recv = self.noise_recv.lock().unwrap();
            recv.decrypt_vec(&response[1..]).or(Err(Error::Noise))?
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
        let _api_call = self.begin_api_call().await;
        match self
            .query_proto(Request::DeviceInfo(pb::DeviceInfoRequest {}))
            .await?
        {
            Response::DeviceInfo(di) => Ok(di),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Returns which product we are connected to.
    pub fn product(&self) -> Product {
        self.communication.info.product
    }

    fn is_multi_edition(&self) -> bool {
        matches!(
            self.product(),
            crate::Product::BitBox02Multi | crate::Product::BitBox02NovaMulti
        )
    }

    /// Returns the firmware version.
    pub fn version(&self) -> &semver::Version {
        &self.communication.info.version
    }

    /// Returns the hex-encoded 4-byte root fingerprint.
    pub async fn root_fingerprint(&self) -> Result<String, Error> {
        let _api_call = self.begin_api_call().await;
        self.root_fingerprint_inner().await.map(hex::encode)
    }

    pub(crate) async fn root_fingerprint_inner(&self) -> Result<Vec<u8>, Error> {
        match self
            .query_proto(Request::Fingerprint(pb::RootFingerprintRequest {}))
            .await?
        {
            Response::Fingerprint(pb::RootFingerprintResponse { fingerprint }) => Ok(fingerprint),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Show recovery words on the Bitbox.
    pub async fn show_mnemonic(&self) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        match self
            .query_proto(Request::ShowMnemonic(pb::ShowMnemonicRequest {}))
            .await?
        {
            Response::Success(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Restore from recovery words on the Bitbox.
    pub async fn restore_from_mnemonic(&self) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        let now = std::time::SystemTime::now();
        let duration_since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        match self
            .query_proto(Request::RestoreFromMnemonic(
                pb::RestoreFromMnemonicRequest {
                    timestamp: duration_since_epoch.as_secs() as u32,
                    timezone_offset: chrono::Local::now().offset().local_minus_utc(),
                },
            ))
            .await?
        {
            Response::Success(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Invokes the password change workflow on the device.
    /// Requires firmware version >=9.25.0.
    pub async fn change_password(&self) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        self.validate_version(">=9.25.0")?;
        match self
            .query_proto(Request::ChangePassword(pb::ChangePasswordRequest {}))
            .await?
        {
            Response::Success(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Invokes the BIP85-BIP39 workflow on the device, letting the user select the number of words
    /// (12, 28, 24) and an index and display a derived BIP-39 mnemonic.
    pub async fn bip85_app_bip39(&self) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        self.validate_version(">=9.17.0")?;
        match self
            .query_proto(Request::Bip85(pb::Bip85Request {
                app: Some(pb::bip85_request::App::Bip39(())),
            }))
            .await?
        {
            Response::Bip85(pb::Bip85Response {
                app: Some(pb::bip85_response::App::Bip39(())),
            }) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}

#[cfg(all(test, not(feature = "multithreaded")))]
mod tests {
    use super::*;
    use crate::communication::{Error as CommunicationError, ReadWrite};
    use crate::runtime::DefaultRuntime;
    use crate::util::Threading;
    use async_trait::async_trait;
    use futures_channel::oneshot;
    use prost::Message;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct BlockingState {
        writes: usize,
        reads: usize,
        first_read_started: Option<oneshot::Sender<()>>,
        release_first_read: Option<oneshot::Receiver<()>>,
        response_cipher: CipherState,
    }

    struct BlockingReadWrite {
        state: Rc<RefCell<BlockingState>>,
    }

    impl Threading for BlockingReadWrite {}

    #[async_trait(?Send)]
    impl ReadWrite for BlockingReadWrite {
        fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
            self.state.borrow_mut().writes += 1;
            Ok(msg.len())
        }

        async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
            let (read_index, started, release) = {
                let mut state = self.state.borrow_mut();
                let read_index = state.reads;
                state.reads += 1;
                (
                    read_index,
                    state.first_read_started.take(),
                    state.release_first_read.take(),
                )
            };
            if read_index == 0 {
                if let Some(started) = started {
                    let _ = started.send(());
                }
                if let Some(release) = release {
                    let _ = release.await;
                }
            }

            let response = pb::Response {
                response: Some(Response::DeviceInfo(pb::DeviceInfoResponse {
                    name: "BitBox".into(),
                    initialized: true,
                    version: "9.26.0".into(),
                    mnemonic_passphrase_enabled: false,
                    monotonic_increments_remaining: 42,
                    securechip_model: "ATECC608B".into(),
                    bluetooth: None,
                    password_stretching_algo: "pwhash".into(),
                })),
            }
            .encode_to_vec();
            let encrypted = self
                .state
                .borrow_mut()
                .response_cipher
                .encrypt_vec(&response);
            let mut framed = vec![0x00, RESPONSE_SUCCESS];
            framed.extend_from_slice(&encrypted);
            Ok(framed)
        }
    }

    fn paired_for_test(state: Rc<RefCell<BlockingState>>) -> PairedBitBox<DefaultRuntime> {
        let key = [7u8; 32];
        PairedBitBox {
            communication: communication::HwwCommunication::from_test_parts(
                Box::new(BlockingReadWrite { state }),
                communication::Info {
                    version: semver::Version::parse("9.26.0").unwrap(),
                    product: Product::BitBox02Multi,
                    unlocked: true,
                    initialized: Some(true),
                },
            ),
            api_mutex: AsyncMutex::new(()),
            noise_send: Mutex::new(CipherState::new(&key, 0)),
            noise_recv: Mutex::new(CipherState::new(&key, 0)),
        }
    }

    #[tokio::test]
    async fn paired_api_calls_are_serialized() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let state = Rc::new(RefCell::new(BlockingState {
            writes: 0,
            reads: 0,
            first_read_started: Some(started_tx),
            release_first_read: Some(release_rx),
            response_cipher: CipherState::new(&[7u8; 32], 0),
        }));
        let paired = Rc::new(paired_for_test(Rc::clone(&state)));

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let first = tokio::task::spawn_local({
                    let paired = Rc::clone(&paired);
                    async move { paired.device_info().await }
                });
                started_rx.await.unwrap();
                assert_eq!(state.borrow().writes, 1);

                let second = tokio::task::spawn_local({
                    let paired = Rc::clone(&paired);
                    async move { paired.device_info().await }
                });
                tokio::task::yield_now().await;
                assert_eq!(state.borrow().writes, 1);

                release_tx.send(()).unwrap();
                first.await.unwrap().unwrap();
                second.await.unwrap().unwrap();
                assert_eq!(state.borrow().writes, 2);
            })
            .await;
    }
}
