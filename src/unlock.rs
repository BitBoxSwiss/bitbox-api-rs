// SPDX-License-Identifier: Apache-2.0

use crate::{error::Error, pb, runtime::Runtime, LockedBitBox, Session, UnlockedBitBox};
use futures_util::future::{poll_fn, select, Either};
use futures_util::task::AtomicWaker;
use pb::{request::Request, response::Response, unlock_response::State};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::task::Poll;
use std::time::Duration;

#[cfg(feature = "multithreaded")]
type InputFuture = futures_util::future::BoxFuture<'static, Result<Option<String>, Error>>;
#[cfg(not(feature = "multithreaded"))]
type InputFuture = futures_util::future::LocalBoxFuture<'static, Result<Option<String>, Error>>;

#[cfg(feature = "multithreaded")]
type InputCallback = dyn Fn() -> InputFuture + Send + Sync;
#[cfg(not(feature = "multithreaded"))]
type InputCallback = dyn Fn() -> InputFuture;

#[cfg(feature = "multithreaded")]
type AvailabilityCallback = dyn Fn(Option<HostPassphraseRequest>) + Send + Sync;
#[cfg(not(feature = "multithreaded"))]
type AvailabilityCallback = dyn Fn(Option<HostPassphraseRequest>);

/// Optional host entry for the device's BIP39 passphrase setting (firmware v9.28.0+).
///
/// With only an input callback, host entry is requested automatically once per unlock.
/// Rejection, cancellation or invalid input falls back to device entry. Add an availability
/// callback to let the application decide when to request host entry instead.
///
/// Callbacks run as part of [`LockedBitBox::unlock`] and must not make device
/// queries. The input callback must yield while waiting for the user; GUI applications should
/// send a message to their UI and await its reply. Dropping `unlock` drops that future,
/// so the callback should arrange to dismiss its dialog on cancellation.
///
/// The default configuration uses device entry only. Older firmware unlocks on the device before
/// pairing, without using this configuration.
#[derive(Default)]
pub struct PassphraseConfig {
    enter: Option<Box<InputCallback>>,
    available: Option<Box<AvailabilityCallback>>,
}

impl PassphraseConfig {
    /// Configure async host input, called only after approval on the device.
    /// Return `Ok(None)` to cancel, or `Ok(Some(passphrase))` to submit, including an empty
    /// string. Errors abort unlocking; [`Error::PassphraseInput`] can describe UI errors.
    #[cfg(feature = "multithreaded")]
    pub fn new<F, Fut>(enter_mnemonic_passphrase: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<String>, Error>> + Send + 'static,
    {
        Self {
            enter: Some(Box::new(move || Box::pin(enter_mnemonic_passphrase()))),
            available: None,
        }
    }

    /// Configure async host input, called only after approval on the device.
    /// Return `Ok(None)` to cancel, or `Ok(Some(passphrase))` to submit, including an empty
    /// string. Errors abort unlocking; [`Error::PassphraseInput`] can describe UI errors.
    #[cfg(not(feature = "multithreaded"))]
    pub fn new<F, Fut>(enter_mnemonic_passphrase: F) -> Self
    where
        F: Fn() -> Fut + 'static,
        Fut: Future<Output = Result<Option<String>, Error>> + 'static,
    {
        Self {
            enter: Some(Box::new(move || Box::pin(enter_mnemonic_passphrase()))),
            available: None,
        }
    }

    /// Let the application offer a host-entry control instead of requesting automatically.
    /// `Some(handle)` enables the control; `None` withdraws it. Notify once per device-entry
    /// phase, and again with a fresh handle if entry restarts. This callback must return promptly.
    #[cfg(feature = "multithreaded")]
    pub fn with_on_host_passphrase_available<F>(mut self, callback: F) -> Self
    where
        F: Fn(Option<HostPassphraseRequest>) + Send + Sync + 'static,
    {
        self.available = Some(Box::new(callback));
        self
    }

    /// Let the application offer a host-entry control instead of requesting automatically.
    /// `Some(handle)` enables the control; `None` withdraws it. Notify once per device-entry
    /// phase, and again with a fresh handle if entry restarts. This callback must return promptly.
    #[cfg(not(feature = "multithreaded"))]
    pub fn with_on_host_passphrase_available<F>(mut self, callback: F) -> Self
    where
        F: Fn(Option<HostPassphraseRequest>) + 'static,
    {
        self.available = Some(Box::new(callback));
        self
    }
}

#[derive(Default)]
struct HostEntrySignal {
    requested: AtomicBool,
    waker: AtomicWaker,
}

/// Requests device consent to enter the passphrase on the host.
///
/// Safe to clone and call from a UI thread, even without the `multithreaded` feature.
/// Handles from withdrawn controls have no effect on subsequent entry phases or connections.
#[derive(Clone)]
pub struct HostPassphraseRequest {
    signal: Weak<HostEntrySignal>,
}

impl HostPassphraseRequest {
    /// Queue one request and wake the unlock loop, without doing device I/O.
    /// Repeated calls for the same entry phase coalesce into a single request.
    pub fn request(&self) {
        if let Some(signal) = self.signal.upgrade() {
            signal.requested.store(true, Ordering::Release);
            signal.waker.wake();
        }
    }
}

struct Availability<'a> {
    notify: Option<&'a AvailabilityCallback>,
    current: Option<Arc<HostEntrySignal>>,
}

impl Availability<'_> {
    fn offer(&mut self) {
        if self.current.is_none() {
            if let Some(notify) = self.notify {
                let signal = Arc::new(HostEntrySignal::default());
                let handle = HostPassphraseRequest {
                    signal: Arc::downgrade(&signal),
                };
                self.current = Some(signal);
                notify(Some(handle));
            }
        }
    }

    fn withdraw(&mut self) {
        if self.current.take().is_some() {
            if let Some(notify) = self.notify {
                notify(None);
            }
        }
    }

    async fn wait<R: Runtime>(&self) -> bool {
        let delay = std::pin::pin!(R::sleep(Duration::from_millis(100)));
        let Some(signal) = &self.current else {
            delay.await;
            return false;
        };
        let requested = std::pin::pin!(poll_fn(|cx| {
            signal.waker.register(cx.waker());
            if signal.requested.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }));
        matches!(select(requested, delay).await, Either::Left(_))
    }
}

impl Drop for Availability<'_> {
    fn drop(&mut self) {
        self.withdraw();
    }
}

pub(crate) fn supports_paired_unlock(version: &semver::Version) -> bool {
    *version >= semver::Version::new(9, 28, 0)
}

impl<R: Runtime> LockedBitBox<R> {
    /// Unlocks the device and enables wallet operations.
    ///
    /// Pass [`PassphraseConfig::default`] for device entry, or configure host entry callbacks.
    /// This consumes the locked device. On failure or cancellation, reconnect to try again.
    pub async fn unlock(self, config: PassphraseConfig) -> Result<UnlockedBitBox<R>, Error> {
        self.session.unlock(&config).await?;
        Ok(UnlockedBitBox {
            session: self.session,
        })
    }
}

impl<R: Runtime> Session<R> {
    async fn unlock(&self, config: &PassphraseConfig) -> Result<(), Error> {
        let _api_call = self.begin_api_call().await;
        let result = self.unlock_inner(config).await;
        if result.is_err() {
            // Release an abandoned workflow after a known failure. A dropped future cannot do
            // async cleanup; reconnect also resets the session before starting a new handshake.
            let _ = self.communication.reset_session_once().await;
        }
        result
    }

    async fn unlock_inner(&self, config: &PassphraseConfig) -> Result<(), Error> {
        let mut availability = Availability {
            notify: config.available.as_deref(),
            current: None,
        };
        let mut auto_request = config.enter.is_some() && config.available.is_none();
        let mut consent_requested = false;
        let mut request = Request::Unlock(pb::UnlockRequest {});

        loop {
            let Response::Unlock(reply) = self.query_proto(request).await? else {
                return Err(Error::UnexpectedResponse);
            };
            match State::try_from(reply.state).map_err(|_| Error::UnexpectedResponse)? {
                State::Done => return Ok(()),
                State::PassphrasePending => {
                    consent_requested = if auto_request {
                        // Request only once so rejection/cancellation can fall back to device
                        // entry without immediately prompting for host entry again.
                        auto_request = false;
                        true
                    } else {
                        if config.enter.is_some() {
                            availability.offer();
                        }
                        availability.wait::<R>().await
                    };
                    if consent_requested {
                        availability.withdraw();
                    }
                    request = Request::UnlockContinue(pb::UnlockContinueRequest {
                        request_host_entry: consent_requested,
                    });
                }
                State::PassphraseEntered => {
                    consent_requested = false;
                    availability.withdraw();
                    // Confirmation may still be pending; the host control must stay hidden.
                    request = Request::UnlockContinue(pb::UnlockContinueRequest::default());
                }
                State::HostEntryReady => {
                    if !consent_requested {
                        return Err(Error::UnexpectedResponse);
                    }
                    consent_requested = false;
                    let enter = config.enter.as_ref().ok_or(Error::UnexpectedResponse)?;
                    request = Request::UnlockHostInfo(pb::UnlockHostInfoRequest {
                        passphrase: enter().await?,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
