//! # A page's shell around salon
//!
//! The TV page and the phone page open one [`Connection`]. It runs
//! `salon_core::Client` with what the browser offers:
//!
//! - **the token**, drawn once with `crypto.getRandomValues` and kept in
//!   `localStorage`; storage that is full, broken or forbidden only means
//!   the device arrives as new (C9);
//! - **WebRTC first**: one DataChannel, unordered and without
//!   retransmission, to the host (`POST /salon/offer`). If it does not open
//!   within [`RTC_PATIENCE_MS`], or twice in a row, **WebSocket**
//!   (`/salon/ws`) takes over;
//! - **the clock** (`performance.now()`) and the timers salon asks for;
//! - **waking up**: `visibilitychange`, `pageshow` and `online` tell salon
//!   to check the link at once, since iOS freezes pages in the background;
//! - **the TV key**, fetched from `/salon/tv`, which only this computer gets.
//!
//! The page receives [`Happening`]s, in order, and never sees a socket.

pub mod journal;
mod recovery;
pub mod ui;
pub use recovery::{install_panic_hook, install_tv_diagnostics};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::rc::{Rc, Weak};

use js_sys::{Reflect, Uint8Array};
use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, ClientSendError, DeviceId, Kind, Limits, Ms, Qr,
    Refusal, Status, Timing, Token, TvKey, route,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    BinaryType, MessageEvent, RequestInit, RtcDataChannel, RtcDataChannelInit, RtcDataChannelType,
    RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit, WebSocket,
};

/// How long a WebRTC attempt may take to open before WebSocket is used.
pub const RTC_PATIENCE_MS: i32 = 3_000;
/// After this many WebRTC failures in a row, this page sticks to WebSocket.
const RTC_FAILURES: u32 = 2;
/// A DataChannel with this much waiting is behind: skip a packet rather than
/// pile up (geckos.io drops on any backlog; salon resends what matters).
const RTC_BACKLOG: u32 = 64 * 1024;

/// What the page wants.
pub struct Options {
    /// The game's version; the host refuses another one.
    pub game: u32,
    /// [`Kind::Tv`] for the TV page, [`Kind::Phone`] for a phone page.
    pub kind: Kind,
    pub name: Option<String>,
    /// For trying several phones in one browser: each seat keeps its own
    /// identity (`?seat=2`). `None` on a real phone.
    pub seat: Option<String>,
    /// WebSocket only, without trying WebRTC (`?salon=ws`). The
    /// certification harness uses it: its proxies only carry TCP.
    pub websocket_only: bool,
    /// The device's identity, when the page has a better one than its own
    /// storage: in the iPhone app, the app's `player`, which survives the
    /// WebView losing its storage. `None`: drawn once and kept in
    /// `localStorage`.
    pub token: Option<Token>,
}

impl Options {
    /// The usual options for this page: `seat` and `websocket_only` read
    /// from its address (`?seat=2`, `?salon=ws`), no name. The TV always
    /// uses WebSocket.
    pub fn for_page(game: u32, kind: Kind) -> Options {
        let search = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default();
        let param = |name: &str| {
            search
                .trim_start_matches('?')
                .split('&')
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                .map(str::to_owned)
        };
        Options {
            game,
            kind,
            name: None,
            seat: param("seat").filter(|seat| !seat.is_empty()),
            // The TV is on the host's computer: a local WebSocket loses
            // nothing and carries larger packets than a DataChannel.
            websocket_only: kind == Kind::Tv || param("salon").as_deref() == Some("ws"),
            token: None,
        }
    }
}

/// What happened, for the page.
#[derive(Clone, Debug, PartialEq)]
pub enum Happening<Down> {
    Status(Status),
    /// The host restarted or forgot this device: what it said is void.
    Reset,
    Event(Down),
    State(Down),
}

/// A browser callback kept alive as long as what it listens to.
type Listener = Closure<dyn FnMut(JsValue)>;

enum Transport {
    Rtc {
        pc: RtcPeerConnection,
        channel: RtcDataChannel,
        open: bool,
        _closures: Vec<Listener>,
        _patience: Option<ui::Timer>,
    },
    Ws {
        socket: WebSocket,
        _closures: Vec<Listener>,
    },
}

impl Transport {
    fn send(&self, bytes: &[u8]) {
        match self {
            Transport::Rtc {
                channel,
                open: true,
                ..
            } => {
                if channel.buffered_amount() < RTC_BACKLOG {
                    // A failed send is a lost packet: salon sends again.
                    let _: Result<(), JsValue> = channel.send_with_u8_array(bytes);
                }
            }
            Transport::Ws { socket, .. } if socket.ready_state() == WebSocket::OPEN => {
                let _: Result<(), JsValue> = socket.send_with_u8_array(bytes);
            }
            // Not open yet, or no longer: salon sends again once it is.
            Transport::Rtc { .. } | Transport::Ws { .. } => {}
        }
    }

    /// Detaches every handler before closing, so a late event never reaches
    /// a dropped closure, nor a newer connection.
    fn close(self) {
        match self {
            Transport::Rtc { pc, channel, .. } => {
                channel.set_onopen(None);
                channel.set_onmessage(None);
                channel.set_onclose(None);
                pc.set_onconnectionstatechange(None);
                channel.close();
                pc.close();
            }
            Transport::Ws { socket, .. } => {
                socket.set_onopen(None);
                socket.set_onmessage(None);
                socket.set_onclose(None);
                socket.set_onerror(None);
                let _: Result<(), JsValue> = socket.close();
            }
        }
    }
}

struct Inner<Up, Down> {
    client: Option<Client<Up, Down>>,
    /// The TV page: a refusal may only mean the host has a new key.
    tv: bool,
    transport: Option<Transport>,
    /// Bumped at each new transport: events of an older one are ignored.
    generation: u32,
    rtc_failures: u32,
    websocket_only: bool,
    timer: Option<ui::Timer>,
}

struct Shared<Up, Down> {
    inner: RefCell<Inner<Up, Down>>,
    pending: RefCell<VecDeque<Happening<Down>>>,
    delivering: Cell<bool>,
    on: RefCell<Box<dyn FnMut(Happening<Down>)>>,
    wake: ui::Listeners,
}

impl<Up, Down> Drop for Shared<Up, Down> {
    fn drop(&mut self) {
        let inner = self.inner.get_mut();
        inner.timer = None;
        if let Some(transport) = inner.transport.take() {
            transport.close();
        }
    }
}

/// The page's link to the host. Cheap to clone; every clone is the same link,
/// and its resources are released when the final clone is dropped. Keep one
/// handle in the page or view that owns the connection.
#[must_use = "keep a connection in the page or view that owns its transport"]
pub struct Connection<Up, Down> {
    shared: Rc<Shared<Up, Down>>,
}

impl<Up, Down> Clone for Connection<Up, Down> {
    fn clone(&self) -> Self {
        Connection {
            shared: Rc::clone(&self.shared),
        }
    }
}

fn now() -> Ms {
    let ms = web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now());
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "performance.now() is a small positive number of milliseconds"
    )]
    Ms(ms.max(0.0) as u64)
}

fn random_bytes() -> Option<[u8; 16]> {
    let mut bytes = [0_u8; 16];
    web_sys::window()?
        .crypto()
        .ok()?
        .get_random_values_with_u8_array(&mut bytes)
        .ok()?;
    Some(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn unhex(text: &str) -> Option<[u8; 16]> {
    let mut bytes = [0_u8; 16];
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(bytes)
}

/// The device's token: the stored one, or a new one stored if possible.
fn token(kind: Kind, seat: Option<&str>) -> Token {
    let key = format!(
        "salon.token.{kind:?}{}",
        seat.map(|seat| format!(".{seat}")).unwrap_or_default()
    );
    let storage = web_sys::window().and_then(|window| window.local_storage().ok().flatten());
    if let Some(stored) = storage
        .as_ref()
        .and_then(|storage| storage.get_item(&key).ok().flatten())
        .as_deref()
        .and_then(unhex)
    {
        return Token(stored);
    }
    let fresh = random_bytes().unwrap_or_else(|| {
        // No secure random numbers: still a working, if guessable, identity.
        #[expect(clippy::cast_possible_truncation, reason = "only the low bits matter")]
        let seed = now().0 as u8;
        [seed; 16]
    });
    if let Some(storage) = storage {
        // Unsaved, the identity lasts as long as the page.
        let _: Result<(), JsValue> = storage.set_item(&key, &hex(&fresh));
    }
    Token(fresh)
}

impl<Up: Serialize + 'static, Down: DeserializeOwned + 'static> Connection<Up, Down> {
    /// Opens the link; `on` receives what happens, in order and one at a
    /// time. It may be called during [`Connection::send`] and the other
    /// calls, after they took effect: never while another call of `on` runs.
    pub fn open(
        options: Options,
        on: impl FnMut(Happening<Down>) + 'static,
    ) -> Connection<Up, Down> {
        install_panic_hook();
        if options.kind == Kind::Tv {
            install_tv_diagnostics();
        }
        let shared = Rc::new(Shared {
            inner: RefCell::new(Inner {
                client: None,
                tv: options.kind == Kind::Tv,
                transport: None,
                generation: 0,
                rtc_failures: 0,
                websocket_only: options.websocket_only,
                timer: None,
            }),
            pending: RefCell::new(VecDeque::new()),
            delivering: Cell::new(false),
            on: RefCell::new(Box::new(on)),
            wake: ui::Listeners::default(),
        });
        let connection = Connection { shared };
        connection.listen();
        let token = options
            .token
            .unwrap_or_else(|| token(options.kind, options.seat.as_deref()));
        let config = ClientConfig {
            game: options.game,
            kind: options.kind,
            token,
            name: options.name,
            tv_key: None,
            seed: u64::from_le_bytes(token.0[..8].try_into().unwrap_or([0; 8])),
            timing: Timing::default(),
            limits: Limits::default(),
        };
        if options.kind == Kind::Tv {
            let weak = Rc::downgrade(&connection.shared);
            spawn_local(async move {
                let key = fetch_text(route::TV_KEY, None)
                    .await
                    .as_deref()
                    .and_then(unhex);
                if let Some(shared) = weak.upgrade() {
                    let mut config = config;
                    config.tv_key = key.map(TvKey);
                    shared.inner.borrow_mut().client = Some(Client::new(now(), config));
                    pump(&shared);
                }
            });
        } else {
            connection.shared.inner.borrow_mut().client = Some(Client::new(now(), config));
            pump(&connection.shared);
        }
        connection
    }

    /// Wakes salon when the page comes back or the network returns. Callbacks
    /// only hold a Weak reference; the final handle drop unregisters them.
    fn listen(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        // Without these, the tick still wakes the client, only later.
        for name in ["pageshow", "online"] {
            let weak = Rc::downgrade(&self.shared);
            let _: Result<(), JsValue> =
                self.shared
                    .wake
                    .listen::<web_sys::Event>(&window, name, move |_| wake(&weak));
        }
        if let Some(document) = window.document() {
            let weak = Rc::downgrade(&self.shared);
            let _: Result<(), JsValue> = self.shared.wake.listen::<web_sys::Event>(
                &document,
                "visibilitychange",
                move |_| wake(&weak),
            );
        }
    }

    fn with_client<T>(&self, act: impl FnOnce(&mut Client<Up, Down>) -> T) -> Option<T> {
        let result = self.shared.inner.borrow_mut().client.as_mut().map(act);
        pump(&self.shared);
        result
    }

    /// An event for the host: once, in order, even across reconnections.
    pub fn send(&self, event: &Up) -> Result<(), ClientSendError> {
        self.with_client(|client| client.send(event))
            .unwrap_or(Err(ClientSendError::Full))
    }

    /// This device's latest state; replaces the previous one.
    pub fn set_state(&self, state: &Up) -> Result<(), ClientSendError> {
        self.with_client(|client| client.set_state(state))
            .unwrap_or(Err(ClientSendError::Full))
    }

    /// The player leaves.
    pub fn quit(&self) {
        self.with_client(Client::quit);
    }

    /// "Play here" after being replaced, refused or having quit.
    pub fn resume(&self) {
        self.with_client(Client::resume);
    }

    pub fn status(&self) -> Status {
        self.shared
            .inner
            .borrow()
            .client
            .as_ref()
            .map_or(Status::Connecting, Client::status)
    }

    pub fn device(&self) -> Option<DeviceId> {
        self.shared
            .inner
            .borrow()
            .client
            .as_ref()
            .and_then(Client::device)
    }

    pub fn rtt(&self) -> Option<u64> {
        self.shared
            .inner
            .borrow()
            .client
            .as_ref()
            .and_then(Client::rtt)
    }

    /// The host's clock now (for countdowns in step with the TV).
    pub fn host_time(&self) -> Option<Ms> {
        self.shared
            .inner
            .borrow()
            .client
            .as_ref()
            .and_then(|client| client.host_time(now()))
    }
}

fn wake<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(weak: &Weak<Shared<Up, Down>>) {
    let visible = web_sys::window()
        .and_then(|window| window.document())
        .is_none_or(|document| !document.hidden());
    if let Some(shared) = weak.upgrade().filter(|_| visible) {
        if let Ok(mut inner) = shared.inner.try_borrow_mut()
            && let Some(client) = inner.client.as_mut()
        {
            client.wake(now());
        }
        pump(&shared);
    }
}

/// Carries out what salon asks, then hands what happened to the page. Safe
/// to call from anywhere, including from the page's own handler.
fn pump<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(shared: &Rc<Shared<Up, Down>>) {
    {
        let Ok(mut inner) = shared.inner.try_borrow_mut() else {
            return;
        };
        let inner = &mut *inner;
        let now = now();
        while let Some(output) = inner.client.as_mut().and_then(|client| client.poll(now)) {
            match output {
                ClientOutput::Connect => connect(shared, inner),
                ClientOutput::Transmit(bytes) => {
                    if let Some(transport) = &inner.transport {
                        transport.send(&bytes);
                    }
                }
                ClientOutput::Disconnect => {
                    if let Some(transport) = inner.transport.take() {
                        transport.close();
                    }
                    // Whatever that transport still had pending (the WebRTC
                    // patience timer) now belongs to no attempt.
                    inner.generation = inner.generation.wrapping_add(1);
                }
                ClientOutput::Status(status) => {
                    if status == Status::Refused(Refusal::NotAllowed) && inner.tv {
                        refresh_tv_key(shared);
                    }
                    shared
                        .pending
                        .borrow_mut()
                        .push_back(Happening::Status(status));
                }
                ClientOutput::Reset => shared.pending.borrow_mut().push_back(Happening::Reset),
                ClientOutput::Event(event) => shared
                    .pending
                    .borrow_mut()
                    .push_back(Happening::Event(event)),
                ClientOutput::State(state) => shared
                    .pending
                    .borrow_mut()
                    .push_back(Happening::State(state)),
            }
        }
        schedule(shared, inner, now);
    }
    if shared.delivering.replace(true) {
        return;
    }
    loop {
        let next = shared.pending.borrow_mut().pop_front();
        let Some(happening) = next else {
            break;
        };
        (shared.on.borrow_mut())(happening);
    }
    shared.delivering.set(false);
}

/// How long a refused TV waits before asking for the key again: a host that
/// restarted without its data folder has a new one.
const TV_KEY_RETRY_MS: i32 = 2_000;

/// Fetches the TV key again and reconnects with it, after a pause.
fn refresh_tv_key<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    shared: &Rc<Shared<Up, Down>>,
) {
    let weak = Rc::downgrade(shared);
    spawn_local(async move {
        // Until the key comes (the host may be restarting): ask again.
        loop {
            sleep(TV_KEY_RETRY_MS).await;
            let key = fetch_text(route::TV_KEY, None)
                .await
                .as_deref()
                .and_then(unhex);
            let Some(shared) = weak.upgrade() else {
                return;
            };
            let refused = shared.inner.try_borrow().is_ok_and(|inner| {
                inner
                    .client
                    .as_ref()
                    .is_some_and(|client| client.status() == Status::Refused(Refusal::NotAllowed))
            });
            if !refused {
                return;
            }
            if let Some(key) = key
                && let Ok(mut inner) = shared.inner.try_borrow_mut()
                && let Some(client) = inner.client.as_mut()
            {
                client.set_tv_key(TvKey(key));
                client.resume();
                drop(inner);
                pump(&shared);
                return;
            }
        }
    });
}

/// Waits `ms` milliseconds.
#[expect(
    clippy::disallowed_methods,
    reason = "one-shot native Promise resolver has no Rust callback and ends even if its waiter is dropped"
)]
async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _: Result<i32, JsValue> =
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _: Result<JsValue, JsValue> = JsFuture::from(promise).await;
}

fn schedule<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    shared: &Rc<Shared<Up, Down>>,
    inner: &mut Inner<Up, Down>,
    now: Ms,
) {
    inner.timer = None;
    let Some(at) = inner.client.as_ref().and_then(Client::next_deadline) else {
        return;
    };
    let wait = i32::try_from(at.since(now)).unwrap_or(i32::MAX).max(0);
    let weak = Rc::downgrade(shared);
    inner.timer = ui::Timer::after(wait, move || {
        if let Some(shared) = weak.upgrade() {
            shared.inner.borrow_mut().timer = None;
            pump(&shared);
        }
    })
    .ok();
}

/// Calls `act` with the client if the event comes from the current transport.
fn on_transport<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    weak: &Weak<Shared<Up, Down>>,
    generation: u32,
    act: impl FnOnce(&mut Inner<Up, Down>, Ms),
) {
    let Some(shared) = weak.upgrade() else {
        return;
    };
    {
        let Ok(mut inner) = shared.inner.try_borrow_mut() else {
            return;
        };
        if inner.generation != generation {
            return;
        }
        act(&mut inner, now());
    }
    pump(&shared);
}

fn connect<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    shared: &Rc<Shared<Up, Down>>,
    inner: &mut Inner<Up, Down>,
) {
    if let Some(old) = inner.transport.take() {
        old.close();
    }
    inner.generation = inner.generation.wrapping_add(1);
    let use_rtc = !inner.websocket_only && inner.rtc_failures < RTC_FAILURES;
    inner.transport = if use_rtc {
        rtc(shared, inner.generation)
    } else {
        None
    }
    .or_else(|| websocket(shared, inner.generation));
    if inner.transport.is_none()
        && let Some(client) = inner.client.as_mut()
    {
        client.closed(now());
    }
}

fn websocket<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    shared: &Rc<Shared<Up, Down>>,
    generation: u32,
) -> Option<Transport> {
    let location = web_sys::window()?.location();
    let scheme = if location.protocol().ok()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let socket = WebSocket::new(&format!(
        "{scheme}://{}{}",
        location.host().ok()?,
        route::SOCKET
    ))
    .ok()?;
    socket.set_binary_type(BinaryType::Arraybuffer);
    let weak = Rc::downgrade(shared);
    let opened = Listener::new(move |_| {
        on_transport(&weak, generation, |inner, now| {
            if let Some(client) = inner.client.as_mut() {
                client.opened(now, Caps::STREAM);
            }
        });
    });
    let weak = Rc::downgrade(shared);
    let message = Listener::new(move |event: JsValue| {
        let Some(data) = event.dyn_ref::<MessageEvent>().map(MessageEvent::data) else {
            return;
        };
        let bytes = Uint8Array::new(&data).to_vec();
        on_transport(&weak, generation, |inner, now| {
            if let Some(client) = inner.client.as_mut() {
                client.received(now, &bytes);
            }
        });
    });
    let weak = Rc::downgrade(shared);
    let closed = Listener::new(move |_| {
        on_transport(&weak, generation, |inner, now| {
            if let Some(transport) = inner.transport.take() {
                transport.close();
            }
            if let Some(client) = inner.client.as_mut() {
                client.closed(now);
            }
        });
    });
    socket.set_onopen(Some(opened.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(closed.as_ref().unchecked_ref()));
    socket.set_onerror(Some(closed.as_ref().unchecked_ref()));
    Some(Transport::Ws {
        socket,
        _closures: vec![opened, message, closed],
    })
}

/// Switches the current attempt from WebRTC to WebSocket.
fn fall_back<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    weak: &Weak<Shared<Up, Down>>,
    generation: u32,
) {
    let Some(shared) = weak.upgrade() else {
        return;
    };
    {
        let Ok(mut inner) = shared.inner.try_borrow_mut() else {
            return;
        };
        let opened = matches!(inner.transport, Some(Transport::Rtc { open: true, .. }));
        if inner.generation != generation || opened {
            return;
        }
        inner.rtc_failures = inner.rtc_failures.saturating_add(1);
        if let Some(old) = inner.transport.take() {
            old.close();
        }
        inner.generation = inner.generation.wrapping_add(1);
        let generation = inner.generation;
        inner.transport = websocket(&shared, generation);
        if inner.transport.is_none()
            && let Some(client) = inner.client.as_mut()
        {
            client.closed(now());
        }
    }
    pump(&shared);
}

fn rtc<Up: Serialize + 'static, Down: DeserializeOwned + 'static>(
    shared: &Rc<Shared<Up, Down>>,
    generation: u32,
) -> Option<Transport> {
    let pc = RtcPeerConnection::new().ok()?;
    let init = RtcDataChannelInit::new();
    init.set_ordered(false);
    init.set_max_retransmits(0);
    let channel = pc.create_data_channel_with_data_channel_dict("salon", &init);
    channel.set_binary_type(RtcDataChannelType::Arraybuffer);

    let weak = Rc::downgrade(shared);
    let opened = Listener::new(move |_| {
        on_transport(&weak, generation, |inner, now| {
            if let Some(Transport::Rtc { open, .. }) = inner.transport.as_mut() {
                *open = true;
            }
            inner.rtc_failures = 0;
            if let Some(client) = inner.client.as_mut() {
                client.opened(now, Caps::DATAGRAM);
            }
        });
    });
    let weak = Rc::downgrade(shared);
    let message = Listener::new(move |event: JsValue| {
        let Some(data) = event.dyn_ref::<MessageEvent>().map(MessageEvent::data) else {
            return;
        };
        let bytes = Uint8Array::new(&data).to_vec();
        on_transport(&weak, generation, |inner, now| {
            if let Some(client) = inner.client.as_mut() {
                client.received(now, &bytes);
            }
        });
    });
    let weak = Rc::downgrade(shared);
    let closed = Listener::new(move |_| {
        on_transport(&weak, generation, |inner, now| {
            if let Some(transport) = inner.transport.take() {
                transport.close();
            }
            if let Some(client) = inner.client.as_mut() {
                client.closed(now);
            }
        });
    });
    channel.set_onopen(Some(opened.as_ref().unchecked_ref()));
    channel.set_onmessage(Some(message.as_ref().unchecked_ref()));
    channel.set_onclose(Some(closed.as_ref().unchecked_ref()));

    // Offer, answer, and a deadline: whatever fails, WebSocket takes over.
    let weak = Rc::downgrade(shared);
    let offering = pc.clone();
    spawn_local(async move {
        if negotiate(&offering).await.is_none() {
            fall_back(&weak, generation);
        }
    });
    let weak = Rc::downgrade(shared);
    let patience = ui::Timer::after(RTC_PATIENCE_MS, move || fall_back(&weak, generation)).ok();
    Some(Transport::Rtc {
        pc,
        channel,
        open: false,
        _closures: vec![opened, message, closed],
        _patience: patience,
    })
}

/// Posts the offer to the host and applies its answer.
async fn negotiate(pc: &RtcPeerConnection) -> Option<()> {
    let offer = JsFuture::from(pc.create_offer()).await.ok()?;
    let sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
        .ok()?
        .as_string()?;
    let local = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    local.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&local))
        .await
        .ok()?;
    let body = serde_json::to_string(&serde_json::json!({ "type": "offer", "sdp": sdp })).ok()?;
    let answer = fetch_text(route::OFFER, Some(&body)).await?;
    let answer: serde_json::Value = serde_json::from_str(&answer).ok()?;
    let remote = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    remote.set_sdp(answer.get("sdp")?.as_str()?);
    JsFuture::from(pc.set_remote_description(&remote))
        .await
        .ok()?;
    Some(())
}

/// GET (or POST `body`) and the text of a successful answer.
async fn fetch_text(url: &str, body: Option<&str>) -> Option<String> {
    let window = web_sys::window()?;
    let init = RequestInit::new();
    if let Some(body) = body {
        init.set_method("POST");
        init.set_body(&JsValue::from_str(body));
    }
    let response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .ok()?;
    let response: web_sys::Response = response.dyn_into().ok()?;
    if !response.ok() {
        return None;
    }
    JsFuture::from(response.text().ok()?)
        .await
        .ok()?
        .as_string()
}

const SVG: &str = "http://www.w3.org/2000/svg";
/// Light modules around a QR code, so a reader finds its edges.
const QR_QUIET: usize = 4;

/// Draws `qr` (`salon_host::qr`) as an `<svg>` of one path, sized by its
/// container. Its colors are the page's: `fill` on `path` for the dark
/// modules, `background` on `svg` for the light ones.
#[expect(
    clippy::disallowed_methods,
    reason = "SVG attributes of a drawing salon makes itself, not names of the game's page"
)]
pub fn qr(qr: &Qr) -> Result<web_sys::Element, JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let svg = document.create_element_ns(Some(SVG), "svg")?;
    let width = usize::try_from(qr.width).unwrap_or(0);
    let size = width.saturating_add(QR_QUIET.saturating_mul(2));
    svg.set_attribute("viewBox", &format!("0 0 {size} {size}"))?;
    svg.set_attribute("shape-rendering", "crispEdges")?;
    svg.set_attribute("aria-hidden", "true")?;
    let mut d = String::new();
    if width > 0 {
        for (y, row) in qr.dark.chunks(width).enumerate() {
            for (x, _) in row.iter().enumerate().filter(|(_, dark)| **dark) {
                let (x, y) = (x.saturating_add(QR_QUIET), y.saturating_add(QR_QUIET));
                let _: std::fmt::Result = write!(d, "M{x} {y}h1v1h-1z");
            }
        }
    }
    let path = document.create_element_ns(Some(SVG), "path")?;
    path.set_attribute("d", &d)?;
    svg.append_child(&path)?;
    Ok(svg)
}

/// Typed values at the browser rendering boundary.
pub mod style;
