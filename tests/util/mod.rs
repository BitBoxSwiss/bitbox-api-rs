// Since each integration test file is compiled independently and not all integration tests use all
// of the util functions, the ones that are not used by all integration test files produce a
// warning.
#![allow(dead_code)]

use bitcoin::hashes::Hash;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use tokio::fs::{self, File};
use tokio::io::{self, AsyncReadExt};

/// BIP32 xprv from BIP39 mnemonic used by the simulator:
/// boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide
pub const SIMULATOR_BIP32_XPRV: &str = "xprv9s21ZrQH143K2qxpAMxVdyeza5dUBxY11XbJ7eKvRF51sQyhiFXgmn4P4ALi3Nf6bcG8cmPDvMMEFiAVjtXsqeZ47PJfBJif7uSYycMsx9c";

pub fn simulator_xprv() -> bitcoin::bip32::Xpriv {
    SIMULATOR_BIP32_XPRV.parse().unwrap()
}

pub fn simulator_xpub_at<C: bitcoin::secp256k1::Signing>(
    secp: &bitcoin::secp256k1::Secp256k1<C>,
    path: &bitcoin::bip32::DerivationPath,
) -> bitcoin::bip32::Xpub {
    bitcoin::bip32::Xpub::from_priv(secp, &simulator_xprv().derive_priv(secp, path).unwrap())
}

#[derive(Serialize, Deserialize)]
struct Simulator {
    url: String,
    sha256: String,
}

struct Server(Child);

impl Server {
    fn launch(filename: &str) -> Self {
        //let mut command = Command::new(filename);

        let mut command = Command::new("stdbuf");
        command
            .arg("-oL") // Line buffering for stdout
            .arg(filename)
            .stdout(std::process::Stdio::piped());

        command.stdout(std::process::Stdio::piped()); // Capture stdout

        let mut child = command.spawn().expect("failed to start server");

        // Take stdout handle from child
        let stdout = child.stdout.take().unwrap();

        // Spawn a thread to process the output, so we can print it indented for clarity.
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => println!("\t\t{line}"),
                    Err(e) => eprintln!("Error reading line: {e}"),
                }
            }
        });

        Self(child)
    }
}

// Kill server on drop.
impl Drop for Server {
    fn drop(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

async fn hashes_match(mut file: File, expected_hash: &str) -> Result<bool, ()> {
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await.map_err(|_| ())?;

    let actual_hash = hex::encode(bitcoin::hashes::sha256::Hash::hash(&buffer));
    Ok(actual_hash == expected_hash)
}

async fn file_not_exist_or_hash_mismatch(filename: &Path, expected_hash: &str) -> Result<bool, ()> {
    match File::open(filename).await {
        Ok(file) => Ok(!hashes_match(file, expected_hash).await?),
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
// Skips the download if the file already exists and has the correct hash.
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
            println!("Downloading simulator: {sim_url}");
            download_file(&simulator.url, &filename)
                .await
                .map_err(|_| ())?;
            fs::set_permissions(&filename, std::fs::Permissions::from_mode(0o755))
                .await
                .map_err(|_| ())?;
            match File::open(&filename).await {
                Ok(file) => {
                    if !hashes_match(file, &simulator.sha256)
                        .await
                        .map_err(|_| ())?
                    {
                        eprintln!(
                            "Hash mismatch for simulator file '{}', expected {}",
                            filename.display(),
                            simulator.sha256
                        );
                        return Err(());
                    }
                }
                Err(_) => return Err(()), // This should never happen as we just created it.
            }
        }
        filenames.push(filename.to_str().unwrap().to_string());
    }

    Ok(filenames)
}

/// Tests on an initialized device, which is not yet seeded.
pub async fn test_simulators_after_pairing(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>),
) {
    let simulator_filenames = if let Some(simulator_filename) = option_env!("SIMULATOR") {
        vec![simulator_filename.into()]
    } else {
        download_simulators().await.unwrap()
    };
    for simulator_filename in simulator_filenames {
        println!();
        println!("\tSimulator tests using {simulator_filename}");
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
        run(&paired_bitbox).await;
    }
}

/// Tests on an initialized/seeded device.
/// The simulator is initialized with the following mnemonic:
/// boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide
pub async fn test_initialized_simulators(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>),
) {
    test_simulators_after_pairing(async |paired_bitbox| {
        assert!(paired_bitbox.restore_from_mnemonic().await.is_ok());
        run(paired_bitbox).await;
    })
    .await
}
