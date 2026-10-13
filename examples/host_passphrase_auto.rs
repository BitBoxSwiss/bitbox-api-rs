// SPDX-License-Identifier: Apache-2.0

//! Request host passphrase entry automatically, then prompt after approval on the device.
//! Requires firmware v9.28.0+ and the optional passphrase setting enabled on the device.

use bitbox_api::{error::Error, runtime::TokioRuntime, PassphraseConfig};
use inquire::{InquireError, Password};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Without an availability callback, host entry is requested once automatically.
    let config = PassphraseConfig::new(|| async {
        let input = tokio::task::spawn_blocking(|| {
            Password::new("Host passphrase:")
                .without_confirmation()
                .with_help_message("Enter submits (empty allowed); Escape or Ctrl-C cancels.")
                .prompt()
        })
        .await
        .map_err(|error| Error::PassphraseInput(error.to_string()))?;
        // Never trim or normalize the user's input. The device validates it.
        match input {
            Ok(passphrase) => Ok(Some(passphrase)),
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
            Err(error) => Err(Error::PassphraseInput(error.to_string())),
        }
    });

    println!("Follow the instructions on the device; approve host entry when prompted.");
    let bitbox = bitbox_api::BitBox::<TokioRuntime>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02()?,
        Box::new(bitbox_api::NoiseConfigNoCache),
    )
    .await?;
    let pairing = bitbox.unlock_and_pair().await?;
    if let Some(code) = pairing.get_pairing_code() {
        println!("Pairing code:\n{code}");
    }
    let unlocked = match pairing.wait_confirm().await? {
        bitbox_api::PairedBitBox::Locked(device) => device.unlock(config).await?,
        bitbox_api::PairedBitBox::Unlocked(device) => device,
        bitbox_api::PairedBitBox::Uninitialized(_) => {
            return Err("Initialize the BitBox first".into())
        }
    };
    println!(
        "Connection ready. Root fingerprint: {}",
        unlocked.root_fingerprint().await?
    );
    Ok(())
}
