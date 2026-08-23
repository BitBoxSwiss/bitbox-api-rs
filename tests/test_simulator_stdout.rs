// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "simulator")]
// Simulator support is available only on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

mod util;

use std::time::Duration;

use util::{parse_simulator_screens, SimulatorScreen, SimulatorStdout, SimulatorStdoutWaitError};

#[test]
fn parses_structured_simulator_screens() {
    let output = r#"USB setup success
CONFIRM SCREEN START
TITLE: High fee
BODY: The fee is 50.0%
the send amount.
Proceed?
CONFIRM SCREEN END
CONFIRM TRANSACTION ADDRESS SCREEN START
AMOUNT: 0.20000000 TBTC
ADDRESS: tb1p abcd efgh
CONFIRM TRANSACTION ADDRESS SCREEN END
CONFIRM TRANSACTION FEE SCREEN START
AMOUNT: 0.30000000 TBTC
FEE: 0.10000000 TBTC
CONFIRM TRANSACTION FEE SCREEN END
STATUS SCREEN START
TITLE: Transaction
confirmed
and complete
STATUS SCREEN END
CONFIRM SWAP SCREEN START
TITLE: Confirm swap
FROM: 1 BTC
on Bitcoin
TO: 10 ETH
on Ethereum
CONFIRM SWAP SCREEN END
"#;

    assert_eq!(
        parse_simulator_screens(output).unwrap(),
        vec![
            SimulatorScreen::Confirm {
                title: "High fee".into(),
                body: "The fee is 50.0%\nthe send amount.\nProceed?".into(),
            },
            SimulatorScreen::TransactionAddress {
                amount: "0.20000000 TBTC".into(),
                address: "tb1p abcd efgh".into(),
            },
            SimulatorScreen::TransactionFee {
                amount: "0.30000000 TBTC".into(),
                fee: "0.10000000 TBTC".into(),
            },
            SimulatorScreen::Status {
                title: "Transaction".into(),
                body: "confirmed\nand complete".into(),
            },
            SimulatorScreen::Swap {
                title: "Confirm swap".into(),
                from: "1 BTC\non Bitcoin".into(),
                to: "10 ETH\non Ethereum".into(),
            },
        ]
    );
}

#[test]
fn screen_json_is_tagged_and_roundtrips() {
    let screen = SimulatorScreen::Confirm {
        title: String::new(),
        body: "Show policy\ndetails?".into(),
    };
    let value = serde_json::to_value(&screen).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "type": "confirm",
            "title": "",
            "body": "Show policy\ndetails?",
        })
    );
    assert_eq!(
        serde_json::from_value::<SimulatorScreen>(value.clone()).unwrap(),
        screen
    );
    let mut unknown_field = value;
    unknown_field
        .as_object_mut()
        .unwrap()
        .insert("unknown".into(), true.into());
    assert!(serde_json::from_value::<SimulatorScreen>(unknown_field).is_err());
}

#[test]
fn reports_incomplete_and_invalid_screen_blocks() {
    let incomplete =
        parse_simulator_screens("CONFIRM SCREEN START\nTITLE: Warning\nBODY: Continue?\n")
            .unwrap_err();
    assert_eq!(incomplete.line, 1);
    assert_eq!(incomplete.message, "missing CONFIRM SCREEN END");

    let missing_body =
        parse_simulator_screens("CONFIRM SCREEN START\nTITLE: Warning\nCONFIRM SCREEN END\n")
            .unwrap_err();
    assert_eq!(missing_body.line, 1);
    assert_eq!(missing_body.message, "missing BODY field");

    for invalid in [
        "CONFIRM SWAP SCREEN START\nFROM: 1 BTC\nTITLE: Swap\nTO: 20 ETH\nCONFIRM SWAP SCREEN END\n",
        "CONFIRM SCREEN START\nunexpected\nTITLE: Title\nBODY: Body\nCONFIRM SCREEN END\n",
        "CONFIRM SCREEN START\nTITLE: Outer\nBODY: Body\nSTATUS SCREEN START\nTITLE: Inner\nSTATUS SCREEN END\nCONFIRM SCREEN END\n",
        "CONFIRM SCREEN START\nTITLE: Title\nBODY: Body\nSTATUS SCREEN END\nCONFIRM SCREEN END\n",
        "FUTURE SCREEN START\nTITLE: Future\nFUTURE SCREEN END\n",
        "FUTURE SCREEN END\n",
    ] {
        assert!(
            parse_simulator_screens(invalid).is_err(),
            "accepted malformed simulator output: {invalid}"
        );
    }
}

#[test]
fn checkpoints_isolate_snapshots_and_wait_for_stability() {
    let stdout = SimulatorStdout::new();
    stdout.record_line("before checkpoint".into());
    let checkpoint = stdout.checkpoint();
    stdout.record_line("first".into());
    stdout.record_line("second".into());

    let snapshot = stdout
        .wait_until_stable(checkpoint, Duration::from_millis(5), Duration::from_secs(1))
        .unwrap();
    assert_eq!(snapshot.lines(), &["first", "second"]);
    assert_eq!(snapshot.raw(), "first\nsecond\n");
    assert_eq!(stdout.snapshot(checkpoint), snapshot);
}

#[test]
fn waits_for_a_complete_terminal_screen() {
    let stdout = SimulatorStdout::new();
    let checkpoint = stdout.checkpoint();
    let writer = stdout.clone();
    let thread = std::thread::spawn(move || {
        for line in [
            "STATUS SCREEN START",
            "TITLE: Transaction",
            "confirmed",
            "STATUS SCREEN END",
        ] {
            writer.record_line(line.into());
        }
    });

    let snapshot = stdout
        .wait_for_terminal_screen(checkpoint, Duration::from_secs(1))
        .unwrap();
    thread.join().unwrap();
    assert_eq!(
        snapshot.screens().unwrap(),
        vec![SimulatorScreen::Status {
            title: "Transaction".into(),
            body: "confirmed".into(),
        }]
    );

    let no_terminal = SimulatorStdout::new();
    let checkpoint = no_terminal.checkpoint();
    assert!(matches!(
        no_terminal.wait_for_terminal_screen(checkpoint, Duration::ZERO),
        Err(SimulatorStdoutWaitError::Timeout(_))
    ));
}
