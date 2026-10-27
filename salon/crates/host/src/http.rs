//! The HTTP side: pages, the TV key, WebRTC offers, WebSocket upgrades. One
//! thread per connection; every read is bounded.

use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use salon_core::{HostConfig, LinkId, TvKey, route};

use crate::clock::Clock;
use crate::room::{self, INBOX, Net, OUTBOX};
use crate::rtc::{self, Rtcs, Signal};
use crate::store::Store;
use crate::{Game, HttpHandler, Options, Request};

/// Largest request head, and largest body (an SDP offer is a few KiB).
const MAX_HEAD: u64 = 16 * 1024;
const MAX_BODY: usize = 64 * 1024;
/// A client that does not finish its request by then is dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Names this computer answers to.
const LOCAL_NAMES: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];
/// Headers a proxy adds: a request carrying one is not from this computer.
const PROXY_HEADERS: [&str; 6] = [
    "forwarded",
    "x-forwarded-for",
    "x-real-ip",
    "via",
    "cf-connecting-ip",
    "true-client-ip",
];

type Diagnostics = crate::journal::Recorder<u32, salon_core::journal::Diagnostic>;

struct Shared {
    diagnostics: Mutex<Option<Diagnostics>>,
    dist: PathBuf,
    pages: Vec<(String, String)>,
    port: u16,
    tv_key: String,
    rtc_address: SocketAddr,
    cert: salon_crypto::DtlsCert,
    signals: SyncSender<Signal>,
    net: SyncSender<Net>,
    links: AtomicU64,
    handler: Option<HttpHandler>,
    app_page: Option<String>,
}

fn os_random<const N: usize>() -> std::io::Result<[u8; N]> {
    let mut bytes = [0; N];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

pub fn serve<G: Game>(options: Options, game: G) -> Result<(), String> {
    salon_crypto::provider().install_process_default();
    let cert =
        salon_crypto::dtls_cert().ok_or("no secure random numbers for the WebRTC certificate")?;
    let port = options.port;
    let listener = TcpListener::bind(("0.0.0.0", port))
        .map_err(|error| format!("cannot listen on port {port}: {error}"))?;
    let udp = UdpSocket::bind(("0.0.0.0", port))
        .map_err(|error| format!("cannot open UDP port {port}: {error}"))?;
    let lan = crate::lan_address().unwrap_or(std::net::IpAddr::from([127, 0, 0, 1]));
    let rtc_address = SocketAddr::new(lan, port);
    let fresh_key =
        os_random::<16>().map_err(|error| format!("no secure random numbers: {error}"))?;
    let epoch = os_random::<4>().map_or(1, u32::from_le_bytes);
    let store = Store::open(options.data.as_deref(), epoch, fresh_key);
    let key = store.tv_key();
    let config = HostConfig {
        game: options.version,
        epoch: store.epoch(),
        tv_key: TvKey(key),
        timing: options.timing,
        limits: options.limits,
        known: store.known(),
    };

    let (net, inbox) = sync_channel(INBOX);
    let (signals, signal_inbox) = sync_channel(64);
    let (outlet, outgoing) = sync_channel(OUTBOX * 16);
    let rtcs = Rtcs {
        socket: udp,
        address: rtc_address,
        signals: signal_inbox,
        outgoing,
        outlet,
        net: net.clone(),
    };
    thread::spawn(move || rtc::run(&rtcs));
    let handler = game.http();
    let tick = options.tick;
    let clock = Clock::start();
    thread::spawn(move || room::run(&inbox, game, config, tick, store, clock));

    let diagnostic_dir = options
        .data
        .as_deref()
        .unwrap_or_else(|| Path::new("data"))
        .join("journals");
    let diagnostics = Diagnostics::start(diagnostic_dir, "panics", options.version)
        .inspect_err(|error| eprintln!("salon: diagnostics unavailable: {error}"))
        .ok();
    let shared = Arc::new(Shared {
        diagnostics: Mutex::new(diagnostics),
        dist: options.dist,
        pages: options.pages,
        port,
        tv_key: hex(&key),
        rtc_address,
        cert,
        signals,
        net,
        links: AtomicU64::new(1),
        handler,
        app_page: options.app_page,
    });
    println!("{}", options.ready);
    println!("  TV (this computer): http://localhost:{port}");
    // Only the console's echo is lost if stdout is gone.
    let _: std::io::Result<()> = std::io::stdout().flush();
    for stream in listener.incoming().flatten() {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            // A broken request only concerns the browser that sent it.
            let _: std::io::Result<()> = handle(stream, &shared);
        });
    }
    Ok(())
}

struct Head {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn headers(&self, name: &str) -> impl Iterator<Item = &str> {
        self.headers
            .iter()
            .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

fn read_head(reader: &mut impl BufRead) -> std::io::Result<Head> {
    let mut limited = reader.take(MAX_HEAD);
    let mut line = String::new();
    limited.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (
        path.to_owned(),
        query.split('#').next().unwrap_or_default().to_owned(),
    );
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if limited.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    Ok(Head {
        method,
        path,
        query,
        headers,
    })
}

/// `localhost`, `127.0.0.1` or `[::1]`, with or without a port.
fn names_this_machine(host: &str) -> bool {
    let name = match host.rsplit_once(':') {
        Some((name, port)) if !port.contains(']') => name,
        _ => host,
    };
    LOCAL_NAMES
        .iter()
        .any(|local| name.eq_ignore_ascii_case(local))
}

/// Whether the request comes from a page of this computer, served by us:
/// loopback peer, one local `Host`, no proxy header, no foreign `Origin`.
/// (Sidequest learned each condition the hard way: a tunnel made every
/// phone an organizer.)
fn from_this_computer(peer: SocketAddr, head: &Head, port: u16) -> bool {
    let mut hosts = head.headers("host");
    let direct = peer.ip().to_canonical().is_loopback()
        && PROXY_HEADERS.iter().all(|name| head.header(name).is_none())
        && hosts.next().is_some_and(names_this_machine)
        && hosts.next().is_none();
    let own_page = head.headers("origin").all(|origin| {
        LOCAL_NAMES.iter().any(|name| {
            origin == format!("http://{name}:{port}")
                || (port == 80 && origin == format!("http://{name}"))
        })
    });
    direct && own_page
}

/// Whether a response carries its body: not for a `HEAD` request.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Payload {
    Included,
    Omitted,
}

impl Payload {
    fn for_method(method: &str) -> Payload {
        if method == "HEAD" {
            Payload::Omitted
        } else {
            Payload::Included
        }
    }
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
    payload: Payload,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    // Cross-origin isolation lets games use WASM threads (manifesto,
    // principle 5); everything the pages load comes from here anyway.
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cache-Control: no-cache\r\nCross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    if payload == Payload::Included {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn handle(mut stream: TcpStream, shared: &Shared) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let peer = stream.peer_addr()?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let head = read_head(&mut reader)?;
    let link = || LinkId(shared.links.fetch_add(1, Ordering::Relaxed));

    let app_page = shared
        .app_page
        .as_deref()
        .filter(|_| head.path == remote_protocol::PATH);
    if (head.path == route::SOCKET || app_page.is_some())
        && let Some(key) = head.header("sec-websocket-key")
    {
        // The client waits for our answer before sending frames, so the
        // reader holds nothing past the headers.
        drop(reader);
        let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
        let answer = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        stream.write_all(answer.as_bytes())?;
        match app_page {
            Some(page) => crate::app::run(stream, page),
            None => crate::ws::run(stream, link(), &shared.net),
        }
        return Ok(());
    }

    let length = head
        .header("content-length")
        .and_then(|length| length.parse::<usize>().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return respond(
            &mut stream,
            413,
            "text/plain",
            b"too large",
            Payload::Included,
        );
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    route(&mut stream, shared, &head, body, peer)
}

/// Diagnostic traffic never enters game input queues untyped. The log
/// is flushed on this HTTP thread before acknowledging its receipt.
fn diagnostic(
    stream: &mut TcpStream,
    shared: &Shared,
    head: &Head,
    body: &[u8],
) -> std::io::Result<()> {
    let mut hosts = head.headers("host");
    let own_page = hosts
        .next()
        .is_some_and(|host| head.header("origin") == Some(format!("http://{host}").as_str()))
        && hosts.next().is_none();
    if !own_page {
        return respond(
            stream,
            403,
            "text/plain",
            b"our pages only",
            Payload::Included,
        );
    }
    let Ok(report) = serde_json::from_slice::<salon_core::journal::Diagnostic>(body) else {
        return respond(
            stream,
            400,
            "text/plain",
            b"invalid diagnostic",
            Payload::Included,
        );
    };
    if report.message().len() > 16_384 || report.page().len() > 1_024 {
        return respond(
            stream,
            400,
            "text/plain",
            b"invalid diagnostic",
            Payload::Included,
        );
    }
    let saved = shared
        .diagnostics
        .lock()
        .ok()
        .and_then(|mut recorder| {
            let recorder = recorder.as_mut()?;
            recorder.record(report.clone()).ok()?;
            recorder.checkpoint("diagnostic").ok()?;
            recorder.flush().ok()
        })
        .is_some();
    if saved {
        match &report {
            salon_core::journal::Diagnostic::Panic(panic) => {
                if let Ok(json) = serde_json::to_string(panic) {
                    eprintln!("salon: browser panic recorded: {json}");
                }
            }
            salon_core::journal::Diagnostic::Marker { page, .. } => {
                eprintln!("salon: bug marker recorded: {page}");
            }
        }
        let _: Result<(), _> = shared.net.try_send(Net::Diagnostic(report));
    }
    respond(
        stream,
        if saved { 200 } else { 503 },
        "text/plain",
        if saved {
            b"recorded"
        } else {
            b"diagnostics unavailable"
        },
        Payload::Included,
    )
}

/// Answers a plain HTTP request: an offer, the TV key, the game's own, or a
/// file.
fn route(
    stream: &mut TcpStream,
    shared: &Shared,
    head: &Head,
    body: Vec<u8>,
    peer: SocketAddr,
) -> std::io::Result<()> {
    let local = from_this_computer(peer, head, shared.port);
    let link = || LinkId(shared.links.fetch_add(1, Ordering::Relaxed));
    match (head.method.as_str(), head.path.as_str()) {
        ("POST", route::DIAGNOSTIC) => diagnostic(stream, shared, head, &body),
        ("POST", route::OFFER) => {
            match rtc::answer(&body, shared.rtc_address, &shared.cert).inspect_err(|error| {
                if std::env::var_os("SALON_LOG").is_some() {
                    eprintln!("salon: offer refused: {error}");
                }
            }) {
                Ok((rtc, answer)) => {
                    if shared
                        .signals
                        .try_send(Signal { link: link(), rtc })
                        .is_ok()
                    {
                        respond(stream, 200, "application/json", &answer, Payload::Included)
                    } else {
                        respond(stream, 503, "text/plain", b"busy", Payload::Included)
                    }
                }
                Err(error) => respond(
                    stream,
                    400,
                    "text/plain",
                    error.as_bytes(),
                    Payload::Included,
                ),
            }
        }
        ("GET", route::TV_KEY) => {
            if local {
                respond(
                    stream,
                    200,
                    "text/plain",
                    shared.tv_key.as_bytes(),
                    Payload::Included,
                )
            } else {
                respond(
                    stream,
                    403,
                    "text/plain",
                    b"only the computer running the game is the TV",
                    Payload::Included,
                )
            }
        }
        (method, path) => {
            let request = Request {
                method: method.to_owned(),
                path: path.to_owned(),
                query: head.query.clone(),
                body,
                local,
            };
            if let Some(response) = shared
                .handler
                .as_ref()
                .and_then(|handler| handler(&request))
            {
                return respond(
                    stream,
                    response.status,
                    &response.content_type,
                    &response.body,
                    Payload::for_method(method),
                );
            }
            if !matches!(method, "GET" | "HEAD") {
                return respond(
                    stream,
                    405,
                    "text/plain",
                    b"method not allowed",
                    Payload::Included,
                );
            }
            match resolve(&shared.dist, &shared.pages, path)
                .and_then(|file| fs::read(&file).ok().map(|body| (file, body)))
            {
                Some((file, body)) => respond(
                    stream,
                    200,
                    content_type(&file),
                    &body,
                    Payload::for_method(method),
                ),
                None => respond(
                    stream,
                    404,
                    "text/plain",
                    b"not found",
                    Payload::for_method(method),
                ),
            }
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Maps a request path to a file under `dist`, refusing anything that would
/// leave it.
fn resolve(dist: &Path, pages: &[(String, String)], path: &str) -> Option<PathBuf> {
    let file = pages
        .iter()
        .find(|(route, _)| route == path)
        .map_or_else(|| path.trim_start_matches('/'), |(_, file)| file.as_str());
    let file = if file.is_empty() { "index.html" } else { file };
    let relative = Path::new(file);
    relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then(|| dist.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(headers: &[(&str, &str)]) -> Head {
        Head {
            method: "GET".to_owned(),
            path: route::TV_KEY.to_owned(),
            query: String::new(),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    /// The query never reaches the file lookup: `/?salon=ws` is the TV page.
    #[test]
    fn a_request_line_splits_path_and_query() {
        let raw = b"GET /play?seat=2&salon=ws HTTP/1.1\r\nHost: localhost:8090\r\nOrigin: http://localhost:8090\r\n\r\n";
        let head = read_head(&mut &raw[..]).unwrap();
        assert_eq!(head.method, "GET");
        assert_eq!(head.path, "/play");
        assert_eq!(head.query, "seat=2&salon=ws");
        assert_eq!(head.header("host"), Some("localhost:8090"));
        let raw = b"GET /?salon=ws HTTP/1.1\r\n\r\n";
        let head = read_head(&mut &raw[..]).unwrap();
        assert_eq!(head.path, "/");
        let dist = Path::new("/srv/dist");
        assert_eq!(
            resolve(dist, &[], &head.path),
            Some(dist.join("index.html"))
        );
    }

    #[test]
    fn files_never_leave_dist() {
        let dist = Path::new("/srv/dist");
        let pages = vec![("/play".to_owned(), "play.html".to_owned())];
        assert_eq!(resolve(dist, &pages, "/"), Some(dist.join("index.html")));
        assert_eq!(resolve(dist, &pages, "/play"), Some(dist.join("play.html")));
        assert_eq!(
            resolve(dist, &pages, "/pkg/game.js"),
            Some(dist.join("pkg/game.js"))
        );
        assert_eq!(resolve(dist, &pages, "/../secret"), None);
        assert_eq!(resolve(dist, &pages, "/pkg/../../secret"), None);
        // A doubled slash stays inside dist.
        assert_eq!(
            resolve(dist, &pages, "//etc/passwd"),
            Some(dist.join("etc/passwd"))
        );
    }

    /// E4: only a page of this computer gets the TV key.
    #[test]
    fn only_this_computer_is_the_tv() {
        let loopback: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let phone: SocketAddr = "192.168.1.20:5000".parse().unwrap();
        let tv = head(&[("Host", "localhost:8090")]);
        assert!(from_this_computer(loopback, &tv, 8090));
        assert!(!from_this_computer(phone, &tv, 8090));
        // Through a tunnel or a proxy on this computer.
        assert!(!from_this_computer(
            loopback,
            &head(&[("Host", "localhost:8090"), ("X-Forwarded-For", "1.2.3.4")]),
            8090
        ));
        assert!(!from_this_computer(
            loopback,
            &head(&[("Host", "game.example.com")]),
            8090
        ));
        // A page of another site, open on this computer.
        assert!(!from_this_computer(
            loopback,
            &head(&[
                ("Host", "localhost:8090"),
                ("Origin", "https://evil.example")
            ]),
            8090
        ));
        assert!(from_this_computer(
            loopback,
            &head(&[
                ("Host", "127.0.0.1:8090"),
                ("Origin", "http://127.0.0.1:8090")
            ]),
            8090
        ));
        // The forms of this computer's names, and look-alikes.
        assert!(from_this_computer(
            loopback,
            &head(&[("Host", "LOCALHOST:8090")]),
            8090
        ));
        assert!(from_this_computer(
            loopback,
            &head(&[("Host", "[::1]:8090")]),
            8090
        ));
        assert!(from_this_computer(
            loopback,
            &head(&[("Host", "[::1]")]),
            8090
        ));
        assert!(!from_this_computer(
            loopback,
            &head(&[("Host", "localhost.evil.example")]),
            8090
        ));
        assert!(!from_this_computer(loopback, &head(&[]), 8090));
        // Two Host headers.
        assert!(!from_this_computer(
            loopback,
            &head(&[("Host", "localhost:8090"), ("Host", "evil")]),
            8090
        ));
    }
}
