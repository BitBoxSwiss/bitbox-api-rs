#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod subtests;

use subtests::PairedBitBox;

use bitcoin::hashes::Hash;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::str::FromStr;
use tokio::fs::{self, File};
use tokio::io::{self, AsyncReadExt};

use bitbox_api::pb;

#[derive(Serialize, Deserialize)]
struct Simulator {
    url: String,
    sha256: String,
}

struct Server(Child);

impl Server {
    fn launch(filename: &str) -> Self {
        Self(
            Command::new(filename)
                .spawn()
                .expect("failed to start server"),
        )
    }
}

// Kill server on drop.
impl Drop for Server {
    fn drop(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

async fn file_not_exist_or_hash_mismatch(filename: &Path, expected_hash: &str) -> Result<bool, ()> {
    match File::open(filename).await {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).await.map_err(|_| ())?;

            let actual_hash = hex::encode(bitcoin::hashes::sha256::Hash::hash(&buffer));

            Ok(actual_hash != expected_hash)
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(()),
    }
}

async fn download_file(url: &str, filename: &Path) -> Result<(), ()> {
    let client = Client::new();
    let resp = client.get(url).send().await.map_err(|_| ())?;
    if resp.status() != reqwest::StatusCode::OK {
        return Err(());
    }

    let mut out = File::create(filename).await.map_err(|_| ())?;
    io::copy(&mut resp.bytes().await.map_err(|_| ())?.as_ref(), &mut out)
        .await
        .map_err(|_| ())?;
    Ok(())
}

// Download BitBox simulators based on testdata/simulators.json to ./simulators/*.
// Skips the download if the file already exists and has the corect hash.
async fn download_simulators() -> Result<Vec<String>, ()> {
    let data = fs::read_to_string("./tests/simulators.json")
        .await
        .map_err(|_| ())?;
    let simulators: Vec<Simulator> = serde_json::from_str(&data).map_err(|_| ())?;

    let mut filenames = Vec::new();
    for simulator in &simulators {
        let sim_url = url::Url::parse(&simulator.url).map_err(|_| ())?;
        let filename =
            PathBuf::from("tests/simulators").join(Path::new(sim_url.path()).file_name().unwrap());
        fs::create_dir_all(filename.parent().unwrap())
            .await
            .map_err(|_| ())?;

        if file_not_exist_or_hash_mismatch(&filename, &simulator.sha256)
            .await
            .map_err(|_| ())?
        {
            println!("Downloading simulator: {}", sim_url);
            download_file(&simulator.url, &filename)
                .await
                .map_err(|_| ())?;
            fs::set_permissions(&filename, std::fs::Permissions::from_mode(0o755))
                .await
                .map_err(|_| ())?;
        }
        filenames.push(filename.to_str().unwrap().to_string());
    }

    Ok(filenames)
}

async fn test_btc(bitbox: &PairedBitBox) {
    subtests::test_btc_psbt::test(bitbox).await;
    // btc_xpub
    {
        let xpub = bitbox
            .btc_xpub(
                pb::BtcCoin::Tbtc,
                &"m/49'/1'/0'".try_into().unwrap(),
                pb::btc_pub_request::XPubType::Ypub,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            xpub.as_str(),
            "ypub6WqXiL3fbDK5QNPe3hN4uSVkEvuE8wXoNCcecgggSuKVpU3Kc4fTvhuLgUhtnbAdaTb9gpz5PQdvzcsKPTLgW2CPkF5ZNRzQeKFT4NSc1xN",
        );
    }
    // btc_address
    {
        let address = bitbox
            .btc_address(
                pb::BtcCoin::Tbtc,
                &"m/84'/1'/0'/1/10".try_into().unwrap(),
                &bitbox_api::btc::make_script_config_simple(
                    pb::btc_script_config::SimpleType::P2wpkh,
                ),
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            address.as_str(),
            "tb1qq064dxjgl9h9wzgsmzy6t6306qew42w9ka02u3"
        );
    }
    // btc_sign_message
    {
        let xpub_str = bitbox
            .btc_xpub(
                pb::BtcCoin::Btc,
                &"m/49'/0'/0'".try_into().unwrap(),
                pb::btc_pub_request::XPubType::Xpub,
                false,
            )
            .await
            .unwrap();
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let pubkey = bitcoin::bip32::Xpub::from_str(&xpub_str)
            .unwrap()
            .derive_pub(
                &secp,
                &"m/0/10".parse::<bitcoin::bip32::DerivationPath>().unwrap(),
            )
            .unwrap()
            .to_pub()
            .0;

        let sign_result = bitbox
            .btc_sign_message(
                pb::BtcCoin::Btc,
                pb::BtcScriptConfigWithKeypath {
                    script_config: Some(bitbox_api::btc::make_script_config_simple(
                        pb::btc_script_config::SimpleType::P2wpkhP2sh,
                    )),
                    keypath: bitbox_api::Keypath::try_from("m/49'/0'/0'/0/10")
                        .unwrap()
                        .to_vec(),
                },
                b"message",
            )
            .await
            .unwrap();

        pubkey
            .verify(
                &secp,
                &bitcoin::secp256k1::Message::from_digest(
                    bitcoin::hashes::sha256d::Hash::hash(
                        b"\x18Bitcoin Signed Message:\n\x07message",
                    )
                    .to_byte_array(),
                ),
                &bitcoin::secp256k1::ecdsa::Signature::from_compact(&sign_result.sig).unwrap(),
            )
            .unwrap();
    }

    subtests::test_cardano::test(bitbox).await;
}

#[tokio::test]
async fn test_device() {
    let simulator_filenames = if let Some(simulator_filename) = option_env!("SIMULATOR") {
        vec![simulator_filename.into()]
    } else {
        download_simulators().await.unwrap()
    };
    for simulator_filename in simulator_filenames {
        {
            println!("Simulator tests using {}", simulator_filename);
            let _server = Server::launch(&simulator_filename);

            let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
            let bitbox = bitbox_api::BitBox::<bitbox_api::runtime::TokioRuntime>::from_simulator(
                None,
                noise_config,
            )
            .await
            .unwrap();
            let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
            let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();

            let device_info = paired_bitbox.device_info().await.unwrap();

            assert_eq!(device_info.name, "My BitBox");
            assert_eq!(paired_bitbox.product(), bitbox_api::Product::BitBox02Multi);

            assert!(paired_bitbox.restore_from_mnemonic().await.is_ok());

            // --- Tests that run on the initialized/seeded device follow.
            // --- The simulator is initialized with the following mnemonic:
            // --- boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide

            assert_eq!(
                paired_bitbox.root_fingerprint().await.unwrap().as_str(),
                "4c00739d"
            );

            assert!(paired_bitbox.show_mnemonic().await.is_ok());

            test_btc(&paired_bitbox).await;
        }
    }
}
