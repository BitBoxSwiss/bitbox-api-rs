// SPDX-License-Identifier: Apache-2.0

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
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulatorScreen {
    Confirm {
        title: String,
        body: String,
    },
    TransactionAddress {
        amount: String,
        address: String,
    },
    TransactionFee {
        amount: String,
        fee: String,
    },
    Status {
        title: String,
        body: String,
    },
    Swap {
        title: String,
        from: String,
        to: String,
    },
}

impl SimulatorScreen {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Status { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulatorScreenParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for SimulatorScreenParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "simulator stdout line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for SimulatorScreenParseError {}

fn parse_error(line: usize, message: impl Into<String>) -> SimulatorScreenParseError {
    SimulatorScreenParseError {
        line,
        message: message.into(),
    }
}

fn parse_screen_fields(
    lines: &[&str],
    block_line: usize,
    fields: &[&str],
) -> Result<Vec<String>, SimulatorScreenParseError> {
    let mut result = Vec::with_capacity(fields.len());
    let mut line_index = 0;

    for (field_index, field) in fields.iter().enumerate() {
        let Some(line) = lines.get(line_index) else {
            return Err(parse_error(block_line, format!("missing {field} field")));
        };
        let prefix = format!("{field}: ");
        let Some(value) = line.strip_prefix(&prefix) else {
            return Err(parse_error(
                block_line,
                format!("expected {prefix:?}, got {line:?}"),
            ));
        };

        let mut value_lines = vec![value];
        line_index += 1;
        if let Some(next_field) = fields.get(field_index + 1) {
            let next_prefix = format!("{next_field}: ");
            while line_index < lines.len() && !lines[line_index].starts_with(&next_prefix) {
                value_lines.push(lines[line_index]);
                line_index += 1;
            }
        } else {
            value_lines.extend_from_slice(&lines[line_index..]);
            line_index = lines.len();
        }
        result.push(value_lines.join("\n"));
    }

    Ok(result)
}

fn parse_screen_block(
    start: &str,
    lines: &[&str],
    block_line: usize,
) -> Result<SimulatorScreen, SimulatorScreenParseError> {
    match start {
        "CONFIRM SCREEN START" => {
            let fields = parse_screen_fields(lines, block_line, &["TITLE", "BODY"])?;
            Ok(SimulatorScreen::Confirm {
                title: fields[0].clone(),
                body: fields[1].clone(),
            })
        }
        "CONFIRM TRANSACTION ADDRESS SCREEN START" => {
            let fields = parse_screen_fields(lines, block_line, &["AMOUNT", "ADDRESS"])?;
            Ok(SimulatorScreen::TransactionAddress {
                amount: fields[0].clone(),
                address: fields[1].clone(),
            })
        }
        "CONFIRM TRANSACTION FEE SCREEN START" => {
            let fields = parse_screen_fields(lines, block_line, &["AMOUNT", "FEE"])?;
            Ok(SimulatorScreen::TransactionFee {
                amount: fields[0].clone(),
                fee: fields[1].clone(),
            })
        }
        "STATUS SCREEN START" => {
            let Some(title) = lines.first().and_then(|line| line.strip_prefix("TITLE: ")) else {
                return Err(parse_error(block_line, "missing TITLE field"));
            };
            let title = title.to_owned();
            let body = lines[1..].join("\n");
            Ok(SimulatorScreen::Status { title, body })
        }
        "CONFIRM SWAP SCREEN START" => {
            let fields = parse_screen_fields(lines, block_line, &["TITLE", "FROM", "TO"])?;
            Ok(SimulatorScreen::Swap {
                title: fields[0].clone(),
                from: fields[1].clone(),
                to: fields[2].clone(),
            })
        }
        _ => unreachable!(),
    }
}

fn screen_end_marker(start: &str) -> Option<&'static str> {
    match start {
        "CONFIRM SCREEN START" => Some("CONFIRM SCREEN END"),
        "CONFIRM TRANSACTION ADDRESS SCREEN START" => {
            Some("CONFIRM TRANSACTION ADDRESS SCREEN END")
        }
        "CONFIRM TRANSACTION FEE SCREEN START" => Some("CONFIRM TRANSACTION FEE SCREEN END"),
        "STATUS SCREEN START" => Some("STATUS SCREEN END"),
        "CONFIRM SWAP SCREEN START" => Some("CONFIRM SWAP SCREEN END"),
        _ => None,
    }
}

fn is_screen_marker(line: &str) -> bool {
    line.ends_with(" SCREEN START") || line.ends_with(" SCREEN END")
}

pub fn parse_simulator_screens(
    output: &str,
) -> Result<Vec<SimulatorScreen>, SimulatorScreenParseError> {
    let lines: Vec<&str> = output.lines().collect();
    let mut result = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let start = lines[index];
        let Some(end_marker) = screen_end_marker(start) else {
            if is_screen_marker(start) {
                return Err(parse_error(
                    index + 1,
                    format!("unknown screen marker {start:?}"),
                ));
            }
            index += 1;
            continue;
        };
        let mut end = index + 1;
        while end < lines.len() && lines[end] != end_marker {
            if screen_end_marker(lines[end]).is_some() {
                return Err(parse_error(
                    end + 1,
                    format!(
                        "screen starting on line {} contains a nested screen",
                        index + 1
                    ),
                ));
            }
            if is_screen_marker(lines[end]) {
                return Err(parse_error(
                    end + 1,
                    format!(
                        "screen starting on line {} has unexpected marker {:?}",
                        index + 1,
                        lines[end]
                    ),
                ));
            }
            end += 1;
        }
        if end == lines.len() {
            return Err(parse_error(index + 1, format!("missing {end_marker}")));
        }
        result.push(parse_screen_block(
            start,
            &lines[index + 1..end],
            index + 1,
        )?);
        index = end + 1;
    }

    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatorStdoutCheckpoint(usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulatorStdoutSnapshot {
    lines: Vec<String>,
}

impl SimulatorStdoutSnapshot {
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn raw(&self) -> String {
        if self.lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", self.lines.join("\n"))
        }
    }

    pub fn screens(&self) -> Result<Vec<SimulatorScreen>, SimulatorScreenParseError> {
        parse_simulator_screens(&self.raw())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulatorStdoutWaitError {
    Timeout(SimulatorStdoutSnapshot),
    Closed(SimulatorStdoutSnapshot),
}

struct SimulatorStdoutState {
    lines: Vec<String>,
    last_update: Instant,
    closed: bool,
}

struct SimulatorStdoutShared {
    state: Mutex<SimulatorStdoutState>,
    changed: Condvar,
}

#[derive(Clone)]
pub struct SimulatorStdout {
    shared: Arc<SimulatorStdoutShared>,
}

impl SimulatorStdout {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(SimulatorStdoutShared {
                state: Mutex::new(SimulatorStdoutState {
                    lines: Vec::new(),
                    last_update: Instant::now(),
                    closed: false,
                }),
                changed: Condvar::new(),
            }),
        }
    }

    pub(crate) fn record_line(&self, line: String) {
        let mut state = self.shared.state.lock().unwrap();
        state.lines.push(line);
        state.last_update = Instant::now();
        self.shared.changed.notify_all();
    }

    fn mark_closed(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.closed = true;
        self.shared.changed.notify_all();
    }

    pub fn checkpoint(&self) -> SimulatorStdoutCheckpoint {
        SimulatorStdoutCheckpoint(self.shared.state.lock().unwrap().lines.len())
    }

    fn snapshot_locked(
        &self,
        state: &SimulatorStdoutState,
        checkpoint: SimulatorStdoutCheckpoint,
    ) -> SimulatorStdoutSnapshot {
        assert!(checkpoint.0 <= state.lines.len());
        SimulatorStdoutSnapshot {
            lines: state.lines[checkpoint.0..].to_vec(),
        }
    }

    pub fn snapshot(&self, checkpoint: SimulatorStdoutCheckpoint) -> SimulatorStdoutSnapshot {
        let state = self.shared.state.lock().unwrap();
        self.snapshot_locked(&state, checkpoint)
    }

    pub fn wait_until_stable(
        &self,
        checkpoint: SimulatorStdoutCheckpoint,
        stable_for: Duration,
        timeout: Duration,
    ) -> Result<SimulatorStdoutSnapshot, SimulatorStdoutWaitError> {
        let started = Instant::now();
        let deadline = started + timeout;
        let mut state = self.shared.state.lock().unwrap();

        loop {
            let now = Instant::now();
            let last_update = if state.lines.len() > checkpoint.0 {
                state.last_update
            } else {
                started
            };
            let stable_elapsed = now.saturating_duration_since(last_update);
            if stable_elapsed >= stable_for || state.closed {
                return Ok(self.snapshot_locked(&state, checkpoint));
            }
            if now >= deadline {
                return Err(SimulatorStdoutWaitError::Timeout(
                    self.snapshot_locked(&state, checkpoint),
                ));
            }

            let wait_for = std::cmp::min(stable_for - stable_elapsed, deadline - now);
            (state, _) = self.shared.changed.wait_timeout(state, wait_for).unwrap();
        }
    }

    pub fn wait_for_terminal_screen(
        &self,
        checkpoint: SimulatorStdoutCheckpoint,
        timeout: Duration,
    ) -> Result<SimulatorStdoutSnapshot, SimulatorStdoutWaitError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock().unwrap();

        loop {
            let snapshot = self.snapshot_locked(&state, checkpoint);
            if snapshot
                .screens()
                .is_ok_and(|screens| screens.last().is_some_and(SimulatorScreen::is_terminal))
            {
                return Ok(snapshot);
            }
            if state.closed {
                return Err(SimulatorStdoutWaitError::Closed(snapshot));
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(SimulatorStdoutWaitError::Timeout(snapshot));
            }
            (state, _) = self
                .shared
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap();
        }
    }
}

struct Server {
    child: Child,
    stdout: SimulatorStdout,
    stdout_thread: Option<JoinHandle<()>>,
}

impl Server {
    fn launch(filename: &str) -> Self {
        let mut command = Command::new("stdbuf");
        command
            .arg("-oL") // Line buffering for stdout
            .arg(filename)
            .stdout(std::process::Stdio::piped());

        let mut child = command.spawn().expect("failed to start server");
        let stdout = child.stdout.take().unwrap();
        let captured_stdout = SimulatorStdout::new();
        let thread_stdout = captured_stdout.clone();

        // Spawn a thread to process the output, so we can print it indented for clarity.
        let stdout_thread = std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        thread_stdout.record_line(line.clone());
                        println!("\t\t{line}");
                    }
                    Err(e) => eprintln!("Error reading line: {e}"),
                }
            }
            thread_stdout.mark_closed();
        });

        Self {
            child,
            stdout: captured_stdout,
            stdout_thread: Some(stdout_thread),
        }
    }

    fn stdout(&self) -> SimulatorStdout {
        self.stdout.clone()
    }
}

// Kill server on drop.
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(stdout_thread) = self.stdout_thread.take() {
            let _ = stdout_thread.join();
        }
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
pub async fn test_simulators_after_pairing_with_stdout(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>, &SimulatorStdout),
) {
    let simulator_filenames = if let Some(simulator_filename) = option_env!("SIMULATOR") {
        vec![simulator_filename.into()]
    } else {
        download_simulators().await.unwrap()
    };
    for simulator_filename in simulator_filenames {
        println!();
        println!("\tSimulator tests using {simulator_filename}");
        let server = Server::launch(&simulator_filename);
        let stdout = server.stdout();
        let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
        let bitbox = bitbox_api::BitBox::<bitbox_api::runtime::TokioRuntime>::from_simulator(
            None,
            noise_config,
        )
        .await
        .unwrap();
        let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
        let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();
        run(&paired_bitbox, &stdout).await;
    }
}

/// Tests on an initialized device, which is not yet seeded.
pub async fn test_simulators_after_pairing(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>),
) {
    test_simulators_after_pairing_with_stdout(async |paired_bitbox, _stdout| {
        run(paired_bitbox).await;
    })
    .await
}

/// Tests on an initialized/seeded device.
/// The simulator is initialized with the following mnemonic:
/// boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide
pub async fn test_initialized_simulators_with_stdout(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>, &SimulatorStdout),
) {
    test_simulators_after_pairing_with_stdout(async |paired_bitbox, stdout| {
        assert!(paired_bitbox.restore_from_mnemonic().await.is_ok());
        run(paired_bitbox, stdout).await;
    })
    .await
}

/// Tests on an initialized/seeded device.
pub async fn test_initialized_simulators(
    run: impl AsyncFn(&bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>),
) {
    test_initialized_simulators_with_stdout(async |paired_bitbox, _stdout| {
        run(paired_bitbox).await;
    })
    .await
}
