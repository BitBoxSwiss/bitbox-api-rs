// SPDX-License-Identifier: Apache-2.0

//! A host-entry button and password dialog driven by the passphrase callbacks.
//! Requires firmware v9.28.0+ and the optional passphrase setting enabled on the device.

use bitbox_api::{error::Error, runtime::TokioRuntime, PassphraseConfig};
use cursive::traits::{Nameable, Resizable};
use cursive::views::{Dialog, DialogFocus, EditView, TextView};
use cursive::{CbSink, Cursive};
use futures_channel::oneshot;
use zeroize::Zeroize;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn finish_input(ui: &mut Cursive, passphrase: Option<String>) {
    let reply = ui
        .take_user_data::<oneshot::Sender<Option<String>>>()
        .expect("pending passphrase input");
    ui.pop_layer();
    if let Err(mut unsent) = reply.send(passphrase) {
        unsent.zeroize();
    }
}

// The callbacks only send UI messages; they never wait synchronously for user input.
fn config(ui: CbSink) -> PassphraseConfig {
    let input_ui = ui.clone();
    PassphraseConfig::new(move || {
        let ui = input_ui.clone();
        async move {
            let (reply, input) = oneshot::channel();
            ui.send(Box::new(move |ui| {
                ui.set_user_data(reply);
                ui.add_layer(
                    Dialog::around(
                        EditView::new()
                            .secret()
                            .on_submit(|ui, text| {
                                // Never trim or normalize the user's input. The device validates it.
                                finish_input(ui, Some(text.to_owned()));
                            })
                            .fixed_width(40),
                    )
                    .title("Host passphrase (Enter to submit)")
                    .button("Cancel", |ui| finish_input(ui, None)),
                );
            }))
            .map_err(|_| Error::PassphraseInput("UI closed".into()))?;
            input
                .await
                .map_err(|_| Error::PassphraseInput("UI closed".into()))
        }
    })
    .with_on_host_passphrase_available(move |request| {
        let _ = ui.send(Box::new(move |ui| {
            ui.call_on_name("device", |dialog: &mut Dialog| {
                dialog.set_content(TextView::new("Follow the instructions on the device."));
                dialog.clear_buttons();
                if let Some(request) = request {
                    dialog.add_button("Enter on host", move |_| request.request());
                }
                dialog.add_button("Quit", Cursive::quit);
                dialog.set_focus(DialogFocus::Button(0));
            });
        }));
    })
}

fn status(ui: &CbSink, text: String) {
    let _ = ui.send(Box::new(move |ui| {
        ui.call_on_name("device", |dialog: &mut Dialog| {
            dialog.set_content(TextView::new(text));
        });
    }));
}

async fn connect(ui: &CbSink) -> Result<String> {
    let bitbox = bitbox_api::BitBox::<TokioRuntime>::from_hid_device(
        bitbox_api::usb::get_any_bitbox02()?,
        Box::new(bitbox_api::NoiseConfigNoCache),
    )
    .await?;
    let pairing = bitbox.unlock_and_pair().await?;
    if let Some(code) = pairing.get_pairing_code() {
        status(
            ui,
            format!("Confirm the pairing code on the device:\n{code}"),
        );
    }
    let paired = pairing.wait_confirm().await?;
    status(ui, "Follow the instructions on the device.".into());
    let unlocked = match paired {
        bitbox_api::PairedBitBox::Locked(device) => device.unlock(config(ui.clone())).await?,
        bitbox_api::PairedBitBox::Unlocked(device) => device,
        bitbox_api::PairedBitBox::Uninitialized(_) => {
            return Err("Initialize the BitBox first".into())
        }
    };
    Ok(unlocked.root_fingerprint().await?)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Cursive owns terminal input on its thread; device communication stays asynchronous.
    let (ready, started) = oneshot::channel();
    let mut terminal = tokio::task::spawn_blocking(move || {
        let mut ui = cursive::default();
        ui.add_layer(
            Dialog::text("Follow the instructions on the device.")
                .title("BitBox02 host passphrase")
                .button("Quit", Cursive::quit)
                .with_name("device"),
        );
        let _ = ready.send(ui.cb_sink().clone());
        ui.try_run().map_err(|error| error.to_string())
    });
    let ui = started.await?;
    let result = tokio::select! {
        result = connect(&ui) => result,
        result = &mut terminal => {
            // Closing the UI also drops the connection future and any pending input.
            result??;
            return Ok(());
        }
    };
    status(
        &ui,
        match &result {
            Ok(fingerprint) => format!("Connection ready. Root fingerprint: {fingerprint}"),
            Err(error) => format!("Connection failed: {error}"),
        },
    );
    terminal.await??;
    Ok(())
}
