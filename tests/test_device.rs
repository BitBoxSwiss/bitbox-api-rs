// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod util;

use util::{test_initialized_simulators, test_simulators_after_pairing};

#[tokio::test]
async fn test_reconnect() {
    use bitbox_api::{btc, pb, runtime::TokioRuntime, BitBox, NoiseConfigNoCache, PairedBitBox};
    use bitbox_api::{CommunicationError, ReadWrite};
    use std::sync::{Arc, Mutex};

    // Record real simulator responses without replacing the transport or firmware behavior.
    struct RecordingTransport {
        transport: Box<bitbox_api::simulator::TcpClient>,
        responses: Arc<Mutex<Vec<u8>>>,
    }

    impl bitbox_api::Threading for RecordingTransport {}

    #[cfg_attr(feature = "multithreaded", async_trait::async_trait)]
    #[cfg_attr(not(feature = "multithreaded"), async_trait::async_trait(?Send))]
    impl ReadWrite for RecordingTransport {
        fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
            self.transport.write(msg)
        }

        async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
            let response = self.transport.read().await?;
            self.responses.lock().unwrap().extend_from_slice(&response);
            Ok(response)
        }
    }

    async fn connect() -> PairedBitBox<TokioRuntime> {
        BitBox::from_simulator(None, Box::new(NoiseConfigNoCache {}))
            .await
            .unwrap()
            .unlock_and_pair()
            .await
            .unwrap()
            .wait_confirm()
            .await
            .unwrap()
    }

    util::test_simulators(async |_stdout| {
        let bitbox = connect().await;
        bitbox.restore_from_mnemonic().await.unwrap();
        if *bitbox.version() >= semver::Version::new(9, 28, 0) {
            // Missing previous transaction data stops the host while the firmware is still
            // waiting for the next signing request.
            let transaction = btc::Transaction {
                script_configs: vec![pb::BtcScriptConfigWithKeypath {
                    script_config: Some(btc::make_script_config_simple(
                        pb::btc_script_config::SimpleType::P2wpkh,
                    )),
                    keypath: bitbox_api::Keypath::try_from("m/84'/0'/0'")
                        .unwrap()
                        .to_vec(),
                }],
                version: 2,
                inputs: vec![btc::TxInput {
                    prev_out_hash: vec![1; 32],
                    prev_out_index: 0,
                    prev_out_value: 100_000,
                    sequence: 0xffffffff,
                    keypath: "m/84'/0'/0'/0/0".try_into().unwrap(),
                    script_config_index: 0,
                    prev_tx: None,
                }],
                outputs: vec![btc::TxOutput::Internal(btc::TxInternalOutput {
                    keypath: "m/84'/0'/0'/1/0".try_into().unwrap(),
                    value: 99_000,
                    script_config_index: 0,
                })],
                locktime: 0,
            };
            let error = bitbox
                .btc_sign(
                    pb::BtcCoin::Btc,
                    &transaction,
                    pb::btc_sign_init_request::FormatUnit::Default,
                )
                .await
                .unwrap_err();
            assert!(matches!(error, bitbox_api::error::Error::BtcSign(message)
                if message == "input's previous transaction required but missing"));
        }
        drop(bitbox);

        // Reconnect to the same running simulator, preserving the firmware session state.
        let responses = Arc::new(Mutex::new(Vec::new()));
        let bitbox = BitBox::<TokioRuntime>::from_transport(
            Box::new(RecordingTransport {
                transport: bitbox_api::simulator::try_connect::<TokioRuntime>(None)
                    .await
                    .unwrap(),
                responses: responses.clone(),
            }),
            Box::new(NoiseConfigNoCache {}),
        )
        .await
        .unwrap();
        responses.lock().unwrap().clear();
        let bitbox = bitbox
            .unlock_and_pair()
            .await
            .unwrap()
            .wait_confirm()
            .await
            .unwrap();
        // The first reply must be a two-byte HWW ACK + unlock SUCCESS. An unfinished
        // workflow would return its encrypted error, which unlock_and_pair currently ignores.
        assert_eq!(&responses.lock().unwrap()[5..9], &[0, 2, 0, 0]);
        assert_eq!(bitbox.root_fingerprint().await.unwrap(), "4c00739d");
    })
    .await
}

#[tokio::test]
async fn test_device_info() {
    test_simulators_after_pairing(async |paired_bitbox| {
        let device_info = paired_bitbox.device_info().await.unwrap();

        // Since v9.24.0, the simulator simulates a Nova device.
        if semver::VersionReq::parse(">=9.24.0")
            .unwrap()
            .matches(paired_bitbox.version())
        {
            assert_eq!(
                paired_bitbox.product(),
                bitbox_api::Product::BitBox02NovaMulti
            );
            assert_eq!(device_info.name, "BitBox HCXT")
        } else {
            assert_eq!(paired_bitbox.product(), bitbox_api::Product::BitBox02Multi);
            assert_eq!(device_info.name, "My BitBox")
        }
    })
    .await
}

#[tokio::test]
async fn test_root_fingerprint() {
    test_initialized_simulators(async |paired_bitbox| {
        assert_eq!(
            paired_bitbox.root_fingerprint().await.unwrap().as_str(),
            "4c00739d"
        );
    })
    .await
}

#[tokio::test]
async fn test_change_password() {
    test_initialized_simulators(async |bitbox| {
        if semver::VersionReq::parse(">=9.25.0")
            .unwrap()
            .matches(bitbox.version())
        {
            assert!(bitbox.change_password().await.is_ok());
        } else {
            assert!(matches!(
                bitbox.change_password().await,
                Err(bitbox_api::error::Error::Version(">=9.25.0"))
            ));
        }
    })
    .await
}
