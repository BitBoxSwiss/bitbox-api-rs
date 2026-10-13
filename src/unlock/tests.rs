// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::communication::{Error as CommunicationError, ReadWrite};
use crate::{
    BitBox, CipherState, HandshakeState, NoiseConfig, NoiseConfigData, PairedBitBox, PairingBitBox,
};
use async_trait::async_trait;
use futures_channel::oneshot;
use noise_protocol::DH;
use prost::Message;
use std::collections::VecDeque;
use std::sync::Mutex;

async fn poll_once<F: Future>(mut future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}

struct TestRuntime;

#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature = "multithreaded"), async_trait(?Send))]
impl Runtime for TestRuntime {
    async fn sleep(_: Duration) {
        tokio::task::yield_now().await;
    }
}

enum Reply {
    Proto(Response),
    Malformed,
    ReadError,
    Pending,
}

fn state(state: State) -> Reply {
    Reply::Proto(Response::Unlock(pb::UnlockResponse {
        state: state as i32,
    }))
}

struct Peer {
    version: String,
    initialized: Option<bool>,
    unlocked: bool,
    legacy_response: Vec<u8>,
    codec: crate::u2fframing::U2fHid,
    incoming: Vec<u8>,
    outgoing: Vec<u8>,
    handshake: HandshakeState,
    ciphers: Option<(CipherState, CipherState)>,
    script: VecDeque<Reply>,
    commands: Vec<u8>,
    requests: Vec<Request>,
    read_error: bool,
    pending: bool,
    pairing_response: u8,
    require_pairing: bool,
}

impl Peer {
    fn respond(&mut self, request: &[u8]) -> Vec<u8> {
        self.commands.push(if request[0] == 0 {
            request[1]
        } else {
            request[0]
        });
        match request {
            [b'i'] => {
                let version = format!("v{}", self.version);
                let mut info = vec![version.len() as u8];
                info.extend_from_slice(version.as_bytes());
                info.extend_from_slice(&[0, 0, u8::from(self.unlocked)]);
                if let Some(initialized) = self.initialized {
                    info.push(u8::from(initialized));
                }
                info
            }
            [3] => vec![0],
            [0, b'h'] => vec![0, 0],
            [0, b'u'] => {
                let mut response = vec![0];
                response.extend_from_slice(&self.legacy_response);
                response
            }
            [0, b'v'] => vec![0, self.pairing_response],
            [0, b'H', message @ ..] => {
                self.handshake.read_message_vec(message).unwrap();
                let mut response = vec![0, 0];
                if self.handshake.completed() {
                    self.ciphers = Some(self.handshake.get_ciphers());
                    response.push(u8::from(self.require_pairing));
                } else {
                    response.extend_from_slice(&self.handshake.write_message_vec(b"").unwrap());
                }
                response
            }
            [0, b'n', message @ ..] => {
                let (receive, send) = self.ciphers.as_mut().unwrap();
                let plaintext = receive.decrypt_vec(message).unwrap();
                let request = pb::Request::decode(plaintext.as_slice())
                    .unwrap()
                    .request
                    .unwrap();
                self.requests.push(request);
                let plaintext = match self.script.pop_front().unwrap() {
                    Reply::Proto(response) => pb::Response {
                        response: Some(response),
                    }
                    .encode_to_vec(),
                    Reply::Malformed => vec![0xff],
                    Reply::ReadError => {
                        self.read_error = true;
                        return vec![];
                    }
                    Reply::Pending => {
                        self.pending = true;
                        return vec![];
                    }
                };
                let mut response = vec![0, 0];
                response.extend_from_slice(&send.encrypt_vec(&plaintext));
                response
            }
            _ => panic!("unexpected command: {request:?}"),
        }
    }
}

struct Transport(Arc<Mutex<Peer>>);
impl crate::Threading for Transport {}

#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature = "multithreaded"), async_trait(?Send))]
impl ReadWrite for Transport {
    fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
        let mut peer = self.0.lock().unwrap();
        peer.incoming.extend_from_slice(msg);
        if let Some(request) = peer.codec.decode(&peer.incoming).unwrap() {
            peer.incoming.clear();
            let response = peer.respond(&request);
            if !peer.read_error && !peer.pending {
                let mut framed = vec![0; crate::u2fframing::MAX_LEN];
                let len = peer.codec.encode(&response, &mut framed).unwrap();
                framed.truncate(len);
                peer.outgoing = framed;
            }
        }
        Ok(msg.len())
    }

    async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
        let pending = {
            let mut peer = self.0.lock().unwrap();
            if std::mem::take(&mut peer.read_error) {
                return Err(CommunicationError::Read);
            }
            if !peer.pending {
                return Ok(std::mem::take(&mut peer.outgoing));
            }
            peer.pending
        };
        assert!(pending);
        futures_util::future::pending().await
    }
}

struct CachedNoise;
impl crate::Threading for CachedNoise {}
impl NoiseConfig for CachedNoise {
    fn read_config(&self) -> Result<NoiseConfigData, crate::ConfigError> {
        let key = noise_rust_crypto::sensitive::Sensitive::from([7u8; 32].into());
        Ok(NoiseConfigData {
            app_static_privkey: Some([8; 32]),
            device_static_pubkeys: vec![noise_rust_crypto::X25519::pubkey(&key).to_vec()],
        })
    }
}

struct Fixture(Arc<Mutex<Peer>>);

impl Fixture {
    fn new(version: &str, script: Vec<Reply>) -> Self {
        Self(Arc::new(Mutex::new(Peer {
            version: version.into(),
            initialized: (semver::Version::parse(version).unwrap()
                >= semver::Version::new(9, 20, 0))
            .then_some(true),
            unlocked: false,
            legacy_response: vec![0],
            codec: crate::u2fframing::U2fHid::new(crate::communication::FIRMWARE_CMD),
            incoming: vec![],
            outgoing: vec![],
            handshake: HandshakeState::new(
                noise_protocol::patterns::noise_xx(),
                false,
                b"Noise_XX_25519_ChaChaPoly_SHA256",
                Some(noise_rust_crypto::sensitive::Sensitive::from(
                    [7u8; 32].into(),
                )),
                None,
                None,
                None,
            ),
            ciphers: None,
            script: script.into(),
            commands: vec![],
            requests: vec![],
            read_error: false,
            pending: false,
            pairing_response: 0,
            require_pairing: true,
        })))
    }

    async fn pair(&self) -> PairingBitBox<TestRuntime> {
        BitBox::from_transport(Box::new(Transport(self.0.clone())), Box::new(CachedNoise))
            .await
            .unwrap()
            .unlock_and_pair()
            .await
            .unwrap()
    }

    async fn locked(&self) -> LockedBitBox<TestRuntime> {
        let PairedBitBox::Locked(device) = self.pair().await.wait_confirm().await.unwrap() else {
            panic!("expected a locked device");
        };
        device
    }

    async fn connect(
        &self,
        config: PassphraseConfig,
    ) -> Result<UnlockedBitBox<TestRuntime>, Error> {
        self.locked().await.unlock(config).await
    }

    fn requests(&self) -> Vec<Request> {
        self.0.lock().unwrap().requests.clone()
    }
}

fn start() -> Request {
    Request::Unlock(pb::UnlockRequest {})
}
fn poll(host: bool) -> Request {
    Request::UnlockContinue(pb::UnlockContinueRequest {
        request_host_entry: host,
    })
}
fn input(value: Option<&str>) -> Request {
    Request::UnlockHostInfo(pb::UnlockHostInfoRequest {
        passphrase: value.map(str::to_owned),
    })
}

#[derive(Debug, PartialEq)]
enum Event {
    Available,
    Withdrawn,
    Input,
}

#[derive(Default)]
struct Ui {
    events: Vec<Event>,
    handles: Vec<HostPassphraseRequest>,
    input: VecDeque<Option<String>>,
}

fn manual(ui: &Arc<Mutex<Ui>>, clicks: usize) -> PassphraseConfig {
    let entry_ui = ui.clone();
    let available_ui = ui.clone();
    PassphraseConfig::new(move || {
        let mut ui = entry_ui.lock().unwrap();
        assert_eq!(ui.events.last(), Some(&Event::Withdrawn));
        ui.events.push(Event::Input);
        let input = ui.input.pop_front().unwrap();
        async move { Ok(input) }
    })
    .with_on_host_passphrase_available(move |handle| {
        let mut ui = available_ui.lock().unwrap();
        match handle {
            Some(handle) => {
                ui.events.push(Event::Available);
                if ui.handles.len() < clicks {
                    handle.request();
                    handle.request();
                }
                // A stale click during a new attempt must never request host entry.
                for old in &ui.handles {
                    old.request();
                }
                ui.handles.push(handle);
            }
            None => ui.events.push(Event::Withdrawn),
        }
    })
}

#[tokio::test]
async fn pairing_returns_the_correct_state() {
    for version in ["7.0.0", "9.19.0", "9.20.0", "9.27.1", "9.28.0"] {
        for initialized in [false, true] {
            for cached in [false, true] {
                let modern = version == "9.28.0";
                let fixture = Fixture::new(version, vec![state(State::Done)]);
                {
                    let mut peer = fixture.0.lock().unwrap();
                    peer.require_pairing = !cached;
                    peer.initialized = peer.initialized.map(|_| initialized);
                    peer.legacy_response = vec![if initialized { 0 } else { 2 }];
                }
                let pairing = fixture.pair().await;
                assert_eq!(pairing.get_pairing_code().is_none(), cached);
                let paired = pairing.wait_confirm().await.unwrap();
                {
                    let peer = fixture.0.lock().unwrap();
                    let expected: &[u8] = match (modern, cached) {
                        (false, false) => b"iuhHHv",
                        (false, true) => b"iuhHH",
                        (true, false) => b"i\x03hHHv",
                        (true, true) => b"i\x03hHH",
                    };
                    assert_eq!(peer.commands, expected);
                    assert!(peer.requests.is_empty());
                }
                match paired {
                    PairedBitBox::Uninitialized(_) => assert!(!initialized),
                    PairedBitBox::Unlocked(_) => assert!(initialized && !modern),
                    PairedBitBox::Locked(device) => {
                        assert!(initialized && modern);
                        let config = PassphraseConfig::new(|| async { panic!("unexpected input") })
                            .with_on_host_passphrase_available(|_| {
                                panic!("unexpected availability")
                            });
                        device.unlock(config).await.unwrap();
                    }
                }
                assert_eq!(
                    fixture.requests(),
                    if modern && initialized {
                        vec![start()]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }
}

#[tokio::test]
async fn legacy_unlock_rejects_failure_and_malformed_responses() {
    for response in [vec![1], vec![], vec![3], vec![0, 0]] {
        let fixture = Fixture::new("9.19.0", vec![]);
        fixture.0.lock().unwrap().legacy_response = response.clone();
        let bitbox = BitBox::<TestRuntime>::from_transport(
            Box::new(Transport(fixture.0.clone())),
            Box::new(CachedNoise),
        )
        .await
        .unwrap();
        let result = bitbox.unlock_and_pair().await;
        if response == [1] {
            assert!(matches!(
                result,
                Err(Error::BitBox(crate::error::BitBoxError::Generic))
            ));
        } else {
            assert!(matches!(result, Err(Error::UnexpectedResponse)));
        }
        assert_eq!(fixture.0.lock().unwrap().commands, b"iu");
    }
}

#[tokio::test]
async fn modern_firmware_requires_initialization_info() {
    let fixture = Fixture::new("9.28.0", vec![]);
    fixture.0.lock().unwrap().initialized = None;
    assert!(matches!(
        fixture.pair().await.wait_confirm().await,
        Err(Error::UnexpectedResponse)
    ));
    assert!(fixture.requests().is_empty());
}

#[tokio::test]
async fn pairing_completes_before_unlock_starts() {
    for cached in [false, true] {
        for unlocked in [false, true] {
            let fixture = Fixture::new("9.28.0", vec![Reply::Pending]);
            {
                let mut peer = fixture.0.lock().unwrap();
                peer.require_pairing = !cached;
                peer.unlocked = unlocked;
            }
            // Pairing returns even when the firmware would wait indefinitely for a password.
            let device = fixture.locked().await;
            assert!(fixture.requests().is_empty());
            let mut unlock = Box::pin(device.unlock(PassphraseConfig::default()));
            assert!(poll_once(unlock.as_mut()).await.is_pending());
            assert_eq!(fixture.requests(), [start()]);
        }
    }
}

#[tokio::test]
async fn rejected_pairing_never_starts_unlock() {
    let fixture = Fixture::new("9.28.0", vec![]);
    fixture.0.lock().unwrap().pairing_response = 1;
    assert!(matches!(
        fixture.pair().await.wait_confirm().await,
        Err(Error::NoisePairingRejected)
    ));
    assert!(fixture.requests().is_empty());
}

#[tokio::test]
async fn device_entry_withdraws_before_confirmation() {
    for clicks in [0, 1] {
        let fixture = Fixture::new(
            "9.28.0",
            vec![
                state(State::PassphrasePending),
                state(State::PassphraseEntered),
                Reply::Pending,
            ],
        );
        let ui = Arc::new(Mutex::new(Ui::default()));
        let locked = fixture.locked().await;
        let mut wait = Box::pin(locked.unlock(manual(&ui, clicks)));
        assert!(poll_once(wait.as_mut()).await.is_pending());
        while fixture.requests().len() < 3 {
            assert!(poll_once(wait.as_mut()).await.is_pending());
        }
        assert_eq!(
            ui.lock().unwrap().events,
            [Event::Available, Event::Withdrawn]
        );
        assert_eq!(fixture.requests(), [start(), poll(clicks > 0), poll(false)]);
        drop(wait);
        assert!(ui.lock().unwrap().handles[0].signal.upgrade().is_none());
    }
}

#[tokio::test]
async fn device_entry_polls_and_reoffers_after_rejection() {
    let fixture = Fixture::new(
        "9.28.0",
        vec![
            state(State::PassphrasePending),
            state(State::PassphrasePending),
            state(State::PassphraseEntered),
            state(State::PassphrasePending),
            state(State::Done),
        ],
    );
    let ui = Arc::new(Mutex::new(Ui::default()));
    fixture.connect(manual(&ui, 0)).await.unwrap();
    assert_eq!(
        ui.lock().unwrap().events,
        [
            Event::Available,
            Event::Withdrawn,
            Event::Available,
            Event::Withdrawn
        ]
    );
    assert_eq!(
        fixture.requests(),
        [start(), poll(false), poll(false), poll(false), poll(false)]
    );
}

#[tokio::test]
async fn host_input_requires_consent_and_preserves_exact_value() {
    for value in [Some("  host passphrase  "), Some(""), None] {
        let fixture = Fixture::new(
            "9.28.0",
            vec![
                state(State::PassphrasePending),
                state(State::HostEntryReady),
                state(State::PassphrasePending),
                state(State::Done),
            ],
        );
        let ui = Arc::new(Mutex::new(Ui {
            input: vec![value.map(str::to_owned)].into(),
            ..Ui::default()
        }));
        fixture.connect(manual(&ui, 1)).await.unwrap();
        assert_eq!(
            fixture.requests(),
            [start(), poll(true), input(value), poll(false)]
        );
        assert_eq!(
            ui.lock().unwrap().events,
            [
                Event::Available,
                Event::Withdrawn,
                Event::Input,
                Event::Available,
                Event::Withdrawn
            ]
        );
        assert!(ui
            .lock()
            .unwrap()
            .handles
            .iter()
            .all(|handle| handle.signal.upgrade().is_none()));
    }
}

#[tokio::test]
async fn consent_rejection_allows_a_fresh_host_request() {
    let fixture = Fixture::new(
        "9.28.0",
        vec![
            state(State::PassphrasePending),
            state(State::PassphrasePending),
            state(State::HostEntryReady),
            state(State::Done),
        ],
    );
    let ui = Arc::new(Mutex::new(Ui {
        input: vec![Some("retry".into())].into(),
        ..Ui::default()
    }));
    fixture.connect(manual(&ui, 2)).await.unwrap();
    assert_eq!(
        fixture.requests(),
        [start(), poll(true), poll(true), input(Some("retry"))]
    );
    assert_eq!(
        ui.lock().unwrap().events,
        [
            Event::Available,
            Event::Withdrawn,
            Event::Available,
            Event::Withdrawn,
            Event::Input
        ]
    );
}

#[tokio::test]
async fn automatic_request_happens_only_once() {
    for (next, value) in [
        (State::HostEntryReady, Some("")),
        (State::HostEntryReady, None),
        (State::HostEntryReady, Some("invalid\ninput")),
        (State::PassphrasePending, None),
        (State::PassphraseEntered, None),
    ] {
        let script = if next == State::HostEntryReady {
            vec![
                state(State::PassphrasePending),
                state(next),
                state(State::PassphrasePending),
                state(State::Done),
            ]
        } else {
            vec![
                state(State::PassphrasePending),
                state(next),
                state(State::Done),
            ]
        };
        let fixture = Fixture::new("9.28.0", script);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let input_calls = calls.clone();
        fixture
            .connect(PassphraseConfig::new(move || {
                input_calls.fetch_add(1, Ordering::Relaxed);
                async move { Ok(value.map(str::to_owned)) }
            }))
            .await
            .unwrap();
        let mut expected = vec![start(), poll(true)];
        if next == State::HostEntryReady {
            expected.push(input(value));
        }
        expected.push(poll(false));
        assert_eq!(fixture.requests(), expected);
        assert_eq!(
            calls.load(Ordering::Relaxed),
            usize::from(next == State::HostEntryReady)
        );
    }
}

#[tokio::test]
async fn no_input_callback_means_device_entry() {
    let fixture = Fixture::new(
        "9.28.0",
        vec![state(State::PassphrasePending), state(State::Done)],
    );
    fixture
        .connect(
            PassphraseConfig::default()
                .with_on_host_passphrase_available(|_| panic!("no host input configured")),
        )
        .await
        .unwrap();
    assert_eq!(fixture.requests(), [start(), poll(false)]);
}

#[tokio::test]
async fn errors_withdraw_controls_and_reset_once() {
    for reply in [
        state(State::HostEntryReady),
        Reply::Proto(Response::Unlock(pb::UnlockResponse { state: 99 })),
        Reply::Proto(Response::Success(pb::Success {})),
        Reply::Proto(Response::Error(pb::Error {
            code: 104,
            message: String::new(),
        })),
        Reply::Malformed,
        Reply::ReadError,
    ] {
        let fixture = Fixture::new("9.28.0", vec![state(State::PassphrasePending), reply]);
        let ui = Arc::new(Mutex::new(Ui::default()));
        assert!(fixture.connect(manual(&ui, 0)).await.is_err());
        assert_eq!(
            ui.lock().unwrap().events,
            [Event::Available, Event::Withdrawn]
        );
        assert!(ui.lock().unwrap().handles[0].signal.upgrade().is_none());
        assert_eq!(
            fixture
                .0
                .lock()
                .unwrap()
                .commands
                .iter()
                .filter(|&&cmd| cmd == 3)
                .count(),
            2
        );
    }
}

#[tokio::test]
async fn input_errors_do_not_submit_a_passphrase() {
    let fixture = Fixture::new(
        "9.28.0",
        vec![
            state(State::PassphrasePending),
            state(State::HostEntryReady),
        ],
    );
    let result = fixture
        .connect(PassphraseConfig::new(|| async {
            Err(Error::PassphraseInput("dialog closed".into()))
        }))
        .await;
    assert!(matches!(result, Err(Error::PassphraseInput(_))));
    assert_eq!(fixture.requests(), [start(), poll(true)]);
    assert_eq!(fixture.0.lock().unwrap().commands.last(), Some(&3));
}

#[tokio::test]
async fn dropping_unlock_withdraws_availability() {
    let fixture = Fixture::new("9.28.0", vec![state(State::PassphrasePending)]);
    let ui = Arc::new(Mutex::new(Ui::default()));
    let locked = fixture.locked().await;
    let mut wait = Box::pin(locked.unlock(manual(&ui, 0)));
    assert!(poll_once(wait.as_mut()).await.is_pending());
    assert_eq!(ui.lock().unwrap().events, [Event::Available]);
    drop(wait);
    assert_eq!(
        ui.lock().unwrap().events,
        [Event::Available, Event::Withdrawn]
    );
    assert!(ui.lock().unwrap().handles[0].signal.upgrade().is_none());
}

#[tokio::test]
async fn host_input_is_async_and_canceled_with_unlock() {
    let fixture = Fixture::new(
        "9.28.0",
        vec![
            state(State::PassphrasePending),
            state(State::HostEntryReady),
        ],
    );
    let (tx, rx) = oneshot::channel::<Option<String>>();
    let rx = Mutex::new(Some(rx));
    let locked = fixture.locked().await;
    let config = PassphraseConfig::new(move || {
        let rx = rx.lock().unwrap().take().unwrap();
        async move { Ok(rx.await.unwrap()) }
    });
    let mut wait = Box::pin(locked.unlock(config));
    assert!(poll_once(wait.as_mut()).await.is_pending());
    tokio::task::yield_now().await;
    assert_eq!(fixture.requests(), [start(), poll(true)]);
    assert!(!tx.is_canceled());
    drop(wait);
    assert!(tx.is_canceled());
}

#[test]
fn a_request_from_another_thread_wakes_the_waiter() {
    struct WaitingRuntime;
    #[cfg_attr(feature = "multithreaded", async_trait)]
    #[cfg_attr(not(feature = "multithreaded"), async_trait(?Send))]
    impl Runtime for WaitingRuntime {
        async fn sleep(_: Duration) {
            futures_util::future::pending().await
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let notify = move |handle| {
        if let Some(handle) = handle {
            tx.send(handle).unwrap();
        }
    };
    let mut availability = Availability {
        notify: Some(&notify),
        current: None,
    };
    availability.offer();
    let handle: HostPassphraseRequest = rx.recv().unwrap();
    #[derive(Default)]
    struct WakeCounter(std::sync::atomic::AtomicUsize);
    impl std::task::Wake for WakeCounter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let counter = Arc::new(WakeCounter::default());
    let waker = std::task::Waker::from(counter.clone());
    let mut context = std::task::Context::from_waker(&waker);
    let mut waiter = std::pin::pin!(availability.wait::<WaitingRuntime>());
    assert!(waiter.as_mut().poll(&mut context).is_pending());
    std::thread::spawn(move || handle.request()).join().unwrap();
    assert_eq!(counter.0.load(Ordering::Relaxed), 1);
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Ready(true));
}

#[tokio::test]
async fn oversized_input_returns_an_error_without_panicking() {
    for len in [8000, 65536] {
        let fixture = Fixture::new(
            "9.28.0",
            vec![
                state(State::PassphrasePending),
                state(State::HostEntryReady),
            ],
        );
        let result = fixture
            .connect(PassphraseConfig::new(move || async move {
                Ok(Some("x".repeat(len)))
            }))
            .await;
        assert!(matches!(
            result,
            Err(Error::Communication(CommunicationError::U2fEncode))
        ));
        assert_eq!(fixture.requests(), [start(), poll(true)]);
    }
}

#[cfg(not(feature = "multithreaded"))]
#[tokio::test]
async fn callbacks_can_capture_local_state() {
    let called = std::rc::Rc::new(std::cell::Cell::new(false));
    let input_called = called.clone();
    let fixture = Fixture::new(
        "9.28.0",
        vec![
            state(State::PassphrasePending),
            state(State::HostEntryReady),
            state(State::Done),
        ],
    );
    fixture
        .connect(PassphraseConfig::new(move || {
            let called = input_called.clone();
            async move {
                called.set(true);
                Ok(Some(String::new()))
            }
        }))
        .await
        .unwrap();
    assert!(called.get());
}

#[cfg(feature = "multithreaded")]
#[tokio::test]
async fn configured_connection_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let config = PassphraseConfig::new(|| async { Ok(None) });
    assert_send_sync(&config);
    let fixture = Fixture::new("9.28.0", vec![state(State::Done)]);
    let pairing = fixture.pair().await;
    assert_send_sync(&pairing);
    let wait = pairing.wait_confirm();
    assert_send(&wait);
    let PairedBitBox::Locked(device) = wait.await.unwrap() else {
        panic!("expected a locked device");
    };
    assert_send_sync(&device);
    let unlock = device.unlock(config);
    assert_send(&unlock);
    assert_send_sync(&unlock.await.unwrap());
}
