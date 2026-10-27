//! # The host's shell around salon
//!
//! One function, [`run`], serves a game to the TV and the phones:
//!
//! - **pages**: the game's files from its `dist` folder, with the headers that
//!   unlock WASM threads (`Cross-Origin-Opener-Policy`,
//!   `Cross-Origin-Embedder-Policy`), never outside that folder;
//! - **WebRTC** at `POST /salon/offer`: the page posts its offer, gets the
//!   answer, and its DataChannel reaches the host's single UDP port (the same
//!   number as the HTTP port) in ICE-lite, at the network address or, for a
//!   page of this computer, at loopback. No internet server, and no C: the
//!   cryptography is `salon-crypto`'s, with one certificate per run;
//! - **WebSocket** at `/salon/ws`, when WebRTC fails, and for the TV;
//! - **the iPhone app** at `/remote` (`remote/CONTRAT.md`), when the game
//!   sets [`Options::app_page`]: the app is sent to the phone page, which
//!   then talks through salon like any phone;
//! - **the TV key** at `GET /salon/tv`, given only to a page of this very
//!   computer (loopback address, local `Host`, no foreign `Origin`, no proxy
//!   header): only the TV can become the TV;
//! - **the room**: one thread owns the [`salon_core::Host`] and the [`Game`],
//!   so the game is plain sequential code that never locks anything.
//!
//! Identities survive a restart: they are saved in the game's data folder,
//! with the epoch, which only grows.
//!
//! The protocol itself, and what it guarantees, is documented in
//! `salon-core`.

// A native program: on wasm32 the crate is empty (see Cargo.toml).
#![cfg(not(target_arch = "wasm32"))]

mod app;
mod clock;
mod http;
mod room;
mod rtc;
mod store;
mod ws;

use std::net::{IpAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub use clock::Clock;
use salon_core::{DeviceEvent, Host, Limits, Ms, Timing};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A game, as the room thread runs it. Everything happens on that one
/// thread, in order.
pub trait Game: Send + 'static {
    /// What devices send (events and states).
    type Up: DeserializeOwned + Send + 'static;
    /// What the host sends to devices.
    type Down: Serialize + Send + 'static;

    /// Once, before any device: add the gamepads ([`Host::add_local`]),
    /// open or close the room.
    fn start(&mut self, now: Ms, host: &mut Host<Self::Up, Self::Down>) {
        let _ = (now, host);
    }

    /// A device arrived, left, came back, sent an event or a state.
    fn on(&mut self, now: Ms, event: DeviceEvent<Self::Up>, host: &mut Host<Self::Up, Self::Down>);

    /// Every [`Options::tick`], and at [`Game::next_deadline`].
    fn tick(&mut self, now: Ms, host: &mut Host<Self::Up, Self::Down>) {
        let _ = (now, host);
    }

    /// When the game needs [`Game::tick`] at the latest (a countdown's end).
    fn next_deadline(&self) -> Option<Ms> {
        None
    }

    /// A request the game answers itself, on an HTTP thread (results to
    /// download, say). `None`: salon serves the files.
    fn http(&self) -> Option<HttpHandler> {
        None
    }
}

/// Answers some HTTP requests itself ([`Game::http`]), on HTTP threads.
pub type HttpHandler = Arc<dyn Fn(&Request) -> Option<Response> + Send + Sync>;

/// An HTTP request a game may answer ([`Game::http`]).
pub struct Request {
    pub method: String,
    /// Without the query.
    pub path: String,
    pub query: String,
    pub body: Vec<u8>,
    /// Sent by this computer itself (the TV), as for the TV key.
    pub local: bool,
}

pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(body: Vec<u8>) -> Response {
        Response {
            status: 200,
            content_type: "application/json".to_owned(),
            body,
        }
    }
}

pub struct Options {
    /// The HTTP port; the UDP port for WebRTC has the same number.
    pub port: u16,
    /// The game's built pages.
    pub dist: PathBuf,
    /// Paths served by one page, besides the files: `("/play", "play.html")`.
    /// `/` serves `index.html` unless listed.
    pub pages: Vec<(String, String)>,
    /// Where identities and the epoch are kept across restarts. `None`: a
    /// restart forgets every device.
    pub data: Option<PathBuf>,
    /// The game's version (`salon_core::HostConfig::game`).
    pub version: u32,
    /// How often [`Game::tick`] runs.
    pub tick: Duration,
    pub timing: Timing,
    pub limits: Limits,
    /// Printed once listening, for the certification harness (`cert.json`
    /// `ready`).
    pub ready: String,
    /// The page the iPhone app opens (`/play`, say). `None`: no `/remote`.
    pub app_page: Option<String>,
}

impl Options {
    pub fn new(port: u16, dist: PathBuf, version: u32, ready: &str) -> Options {
        Options {
            port,
            dist,
            pages: Vec::new(),
            data: None,
            version,
            tick: Duration::from_millis(100),
            timing: Timing::default(),
            limits: Limits::default(),
            ready: ready.to_owned(),
            app_page: None,
        }
    }
}

/// Serves `game` until the process ends. Returns only if it cannot start.
pub fn run<G: Game>(options: Options, game: G) -> Result<(), String> {
    http::serve(options, game)
}

/// The address other devices on the network reach this computer at.
/// Connecting a UDP socket sends nothing; it only asks the OS which
/// interface it would use.
pub fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// The address phones open: `http://<lan address>:<port><path>`.
pub fn join_url(port: u16, path: &str) -> String {
    let host = lan_address().map_or_else(|| "localhost".to_owned(), |ip| ip.to_string());
    format!("http://{host}:{port}{path}")
}

/// A QR code for `url`, as SVG.
pub fn qr_svg(url: &str, dark: &str, light: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(url.as_bytes()).map_err(|error| error.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(320, 320)
        .dark_color(qrcode::render::svg::Color(dark))
        .light_color(qrcode::render::svg::Color(light))
        .build())
}
