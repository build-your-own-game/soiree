//! How a WebSocket dies when it dies badly, end to end: a real server, a real
//! socket, a real `Client`, and between them a relay that plays Toxiproxy's
//! `slow_close` and `reset_peer`. The simulated network of `salon-core` plays
//! the same two cases on a model; this is where TCP has its say. No browser.

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "test: its helpers are outside the #[test] functions Clippy recognizes"
)]

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use nix::libc::linger;
use nix::sys::socket::{setsockopt, sockopt::Linger};
use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, Host, Kind, Limits, Ms, Status, Timing,
    Token, Why, route,
};
use salon_host::{Clock, Game, Options};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const VERSION: u32 = 1;

/// Says each event back, and reports all it hears.
struct Echo {
    heard: Sender<DeviceEvent<u32>>,
}

impl Game for Echo {
    type Up = u32;
    type Down = u32;

    fn on(&mut self, _: Ms, event: DeviceEvent<u32>, host: &mut Host<u32, u32>) {
        if let DeviceEvent::Event { device, event, .. } = &event {
            host.send(*device, event).unwrap();
        }
        // The test may be over: the room outlives it, and must not panic.
        let _: Result<(), _> = self.heard.send(event);
    }
}

/// Serves the echo for the rest of the test run, and gives its port.
fn serve(name: &str, timing: Timing) -> (u16, Receiver<DeviceEvent<u32>>) {
    // Free on TCP and on UDP, which salon opens with the same number.
    let port = loop {
        let tcp = TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            break port;
        }
    };
    let dir = std::env::temp_dir().join(format!("salon-faults-{name}-{port}"));
    let mut options = Options::new(port, dir.clone(), VERSION, "ready");
    options.data = Some(dir);
    options.tick = Duration::from_millis(5);
    options.timing = timing;
    let (heard, game) = channel();
    thread::spawn(move || salon_host::run(options, Echo { heard }));
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        thread::sleep(Duration::from_millis(5));
    }
    (port, game)
}

/// What the relay does to the connections it carries.
#[derive(Default)]
struct Toxics {
    /// How long a side's close is held back from the other side.
    slow_close_ms: AtomicU64,
    /// Bumped to end every connection with a RST, both ways.
    resets: AtomicU32,
    /// What the host sends goes nowhere: Toxiproxy's `timeout`, one way.
    deaf: AtomicBool,
}

/// A TCP relay in front of the host.
struct Relay {
    port: u16,
    toxics: Arc<Toxics>,
}

impl Relay {
    fn in_front_of(host: u16) -> Relay {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let toxics = Arc::<Toxics>::default();
        let shared = Arc::clone(&toxics);
        thread::spawn(move || {
            for phone in listener.incoming() {
                let phone = phone.unwrap();
                let server = TcpStream::connect(("127.0.0.1", host)).unwrap();
                let toxics = Arc::clone(&shared);
                thread::spawn(move || carry(phone, server, &toxics));
            }
        });
        Relay { port, toxics }
    }

    /// Toxiproxy's `slow_close`: a close waits this long before it goes on.
    fn slow_close(&self, delay: Duration) {
        let ms = u64::try_from(delay.as_millis()).unwrap();
        self.toxics.slow_close_ms.store(ms, Ordering::SeqCst);
    }

    /// The page hears nothing from now on.
    fn deafen(&self) {
        self.toxics.deaf.store(true, Ordering::SeqCst);
    }

    fn let_hear(&self) {
        self.toxics.deaf.store(false, Ordering::SeqCst);
    }

    /// Toxiproxy's `reset_peer`, at once.
    fn reset_peer(&self) {
        self.toxics.resets.fetch_add(1, Ordering::SeqCst);
    }
}

/// What happens to what a side sends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fate {
    Arrives,
    Vanishes,
}

/// Copies what is there, if `fate` lets it. `false` when the source closed.
fn pass(from: &mut TcpStream, to: &mut TcpStream, fate: Fate) -> bool {
    let mut buffer = [0; 16 * 1024];
    match from.read(&mut buffer) {
        Ok(0) => false,
        Ok(_) if fate == Fate::Vanishes => true,
        Ok(read) => to.write_all(&buffer[..read]).is_ok(),
        Err(error) => error.kind() == ErrorKind::WouldBlock,
    }
}

/// Ends the connection with a RST instead of a FIN.
fn reset(stream: &TcpStream) {
    let abort = linger {
        l_onoff: 1,
        l_linger: 0,
    };
    setsockopt(stream, Linger, &abort).unwrap();
}

/// Reads and drops what `stream` receives for `delay`.
fn swallow(stream: &mut TcpStream, delay: Duration) {
    let end = Clock::instant() + delay;
    while Clock::instant() < end {
        let mut buffer = [0; 16 * 1024];
        let _: std::io::Result<usize> = stream.read(&mut buffer);
        thread::sleep(Duration::from_millis(1));
    }
}

fn carry(mut phone: TcpStream, mut server: TcpStream, toxics: &Toxics) {
    phone.set_nodelay(true).unwrap();
    server.set_nodelay(true).unwrap();
    // Blocking writes, bounded reads: one thread carries both ways.
    let resets = toxics.resets.load(Ordering::SeqCst);
    phone
        .set_read_timeout(Some(Duration::from_millis(1)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_millis(1)))
        .unwrap();
    loop {
        if toxics.resets.load(Ordering::SeqCst) != resets {
            reset(&phone);
            reset(&server);
            return;
        }
        let delay = Duration::from_millis(toxics.slow_close_ms.load(Ordering::SeqCst));
        if !pass(&mut phone, &mut server, Fate::Arrives) {
            swallow(&mut server, delay);
            return;
        }
        let fate = if toxics.deaf.load(Ordering::SeqCst) {
            Fate::Vanishes
        } else {
            Fate::Arrives
        };
        if !pass(&mut server, &mut phone, fate) {
            swallow(&mut phone, delay);
            return;
        }
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// A page, as far as a transport can tell: a `Client` and a real WebSocket.
struct Page {
    client: Client<u32, u32>,
    port: u16,
    clock: Clock,
    socket: Option<Socket>,
    events: Vec<u32>,
    statuses: Vec<Status>,
    resets: usize,
    /// How each socket that ended ended, as the page's side read it.
    ends: Vec<ErrorKind>,
}

impl Page {
    fn open(port: u16, token: u8, clock: Clock, timing: Timing) -> Page {
        let config = ClientConfig {
            game: VERSION,
            kind: Kind::Phone,
            token: Token([token; 16]),
            name: None,
            tv_key: None,
            seed: u64::from(token),
            timing,
            limits: Limits::default(),
        };
        Page {
            client: Client::new(clock.now(), config),
            port,
            clock,
            socket: None,
            events: Vec::new(),
            statuses: Vec::new(),
            resets: 0,
            ends: Vec::new(),
        }
    }

    fn now(&self) -> Ms {
        self.clock.now()
    }

    fn connect(&mut self, now: Ms) {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_nodelay(true).unwrap();
        let address = format!("ws://127.0.0.1:{}{}", self.port, route::SOCKET);
        let (socket, _) =
            tungstenite::client(address.as_str(), MaybeTlsStream::Plain(stream)).unwrap();
        let MaybeTlsStream::Plain(stream) = socket.get_ref() else {
            panic!("a plain socket");
        };
        stream.set_nonblocking(true).unwrap();
        self.socket = Some(socket);
        self.client.opened(now, Caps::STREAM);
    }

    /// The transport died under the page.
    fn died(&mut self, now: Ms, how: ErrorKind) {
        self.socket = None;
        self.ends.push(how);
        self.client.closed(now);
    }

    fn read(&mut self, now: Ms) {
        while let Some(socket) = &mut self.socket {
            match socket.read() {
                Ok(Message::Binary(bytes)) => self.client.received(now, &bytes),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                    return;
                }
                Err(tungstenite::Error::Io(error)) => self.died(now, error.kind()),
                Err(_) => self.died(now, ErrorKind::UnexpectedEof),
            }
        }
    }

    fn pump(&mut self) {
        let now = self.now();
        self.read(now);
        while let Some(output) = self.client.poll(now) {
            match output {
                ClientOutput::Connect => self.connect(now),
                ClientOutput::Transmit(bytes) => {
                    if let Some(socket) = &mut self.socket
                        && let Err(error) = socket.send(Message::binary(bytes))
                    {
                        let how = match error {
                            tungstenite::Error::Io(error) => error.kind(),
                            _ => ErrorKind::UnexpectedEof,
                        };
                        self.died(now, how);
                    }
                }
                ClientOutput::Disconnect => self.socket = None,
                ClientOutput::Status(status) => self.statuses.push(status),
                ClientOutput::Reset => self.resets += 1,
                ClientOutput::Event(event) => self.events.push(event),
                ClientOutput::State(_) => {}
            }
        }
    }

    /// Runs until `done` holds, at most `ms`. Returns whether it held.
    fn run_until(&mut self, ms: u64, mut done: impl FnMut(&Page) -> bool) -> bool {
        let end = Clock::instant() + Duration::from_millis(ms);
        while Clock::instant() < end {
            self.pump();
            if done(self) {
                return true;
            }
            thread::sleep(Duration::from_millis(1));
        }
        done(self)
    }

    fn run(&mut self, ms: u64) {
        self.run_until(ms, |_| false);
    }

    /// Sends `event` and waits for the game to say it back.
    fn says(&mut self, event: u32) {
        self.client.send(&event).unwrap();
        assert!(
            self.run_until(5_000, |page| page.events.contains(&event)),
            "no echo of {event}: {:?}",
            self.events
        );
    }
}

/// What the game heard so far, for one device: its comings and goings, and
/// its events.
fn sorted(game: &Receiver<DeviceEvent<u32>>) -> (Vec<DeviceEvent<u32>>, Vec<u32>) {
    let mut moves = Vec::new();
    let mut events = Vec::new();
    for heard in game.try_iter() {
        match heard {
            DeviceEvent::Event { event, .. } => events.push(event),
            DeviceEvent::State { .. } => {}
            DeviceEvent::Arrived { .. } | DeviceEvent::Left { .. } | DeviceEvent::Back { .. } => {
                moves.push(heard);
            }
        }
    }
    (moves, events)
}

/// A page whose socket died, or that reloaded, comes back on a new link while
/// the close of the old one is still on its way: that close, when it lands,
/// is about a link salon no longer uses (Colyseus #950).
fn a_late_close_changes_nothing(game: &Receiver<DeviceEvent<u32>>, mut page: Page) {
    assert!(page.run_until(5_000, |page| page.client.status() == Status::Connected));
    assert!(
        !page.statuses.contains(&Status::Replaced),
        "{:?}",
        page.statuses
    );
    let back = page.events.len();
    page.says(2);
    // The close lands here, past FINAL_GRACE too: the old link is gone twice.
    page.run(2_000);
    page.says(3);
    assert_eq!(page.events[back..], [2, 3]);
    assert_eq!(page.client.status(), Status::Connected);
    assert!(
        !page.statuses.contains(&Status::Replaced),
        "{:?}",
        page.statuses
    );
    assert_eq!(page.resets, 0);
    let (moves, events) = sorted(game);
    assert_eq!(
        moves,
        vec![],
        "the game hears nothing: the device never left"
    );
    assert_eq!(events, vec![2, 3]);
}

/// The hole left by reconnexion-39: the FIN of a dead socket reaches the host
/// long after the page came back.
#[test]
fn a_slow_close_does_not_touch_the_link_that_followed() {
    let timing = Timing::default();
    let (host, game) = serve("slow", timing);
    let relay = Relay::in_front_of(host);
    relay.slow_close(Duration::from_millis(700));
    let mut page = Page::open(relay.port, 3, Clock::start(), timing);
    page.says(1);
    let (moves, events) = sorted(&game);
    assert!(
        matches!(moves[..], [DeviceEvent::Arrived { .. }]),
        "{moves:?}"
    );
    assert_eq!(events, vec![1]);

    let now = page.now();
    page.died(now, ErrorKind::ConnectionAborted);
    a_late_close_changes_nothing(&game, page);
}

/// The same close, when a reload is what ended the page: the new one starts
/// from nothing, and must not be told it replaced anybody.
#[test]
fn a_page_reloaded_behind_a_slow_close_just_plays() {
    let timing = Timing::default();
    let (host, game) = serve("reload", timing);
    let relay = Relay::in_front_of(host);
    relay.slow_close(Duration::from_millis(700));
    let clock = Clock::start();
    let mut old = Page::open(relay.port, 4, clock, timing);
    old.says(1);
    let (moves, events) = sorted(&game);
    assert!(
        matches!(moves[..], [DeviceEvent::Arrived { .. }]),
        "{moves:?}"
    );
    assert_eq!(events, vec![1]);

    drop(old);
    a_late_close_changes_nothing(&game, Page::open(relay.port, 4, clock, timing));
}

/// `reset_peer`: no close, no warning, and what was in flight is gone. Here,
/// the worst of it: the game has an event, and its acknowledgement and its echo
/// die with the connection. It is a cut like any other: the device left and
/// came back, each event counted once, each echo reached the page once.
#[test]
fn a_reset_loses_nothing_and_doubles_nothing() {
    let timing = Timing::default();
    let (host, game) = serve("reset", timing);
    let relay = Relay::in_front_of(host);
    let mut page = Page::open(relay.port, 5, Clock::start(), timing);
    page.says(1);
    let mut heard = vec![game.recv_timeout(Duration::from_secs(1)).unwrap()];
    let mut sent = vec![1];
    for round in 1..=3 {
        let event = round * 10;
        relay.deafen();
        page.client.send(&event).unwrap();
        sent.push(event);
        page.pump();
        loop {
            let got = game.recv_timeout(Duration::from_secs(5)).unwrap();
            let arrived = matches!(got, DeviceEvent::Event { event: e, .. } if e == event);
            heard.push(got);
            if arrived {
                break;
            }
        }
        relay.reset_peer();
        relay.let_hear();
        let ends = page.ends.len();
        assert!(
            page.run_until(5_000, |page| page.ends.len() > ends),
            "the page never saw the connection end"
        );
        // Reading or writing, the socket fails: no FIN came, which reads as an end.
        assert_ne!(page.ends.last(), Some(&ErrorKind::UnexpectedEof));
        assert!(page.run_until(5_000, |page| page.client.status() == Status::Connected));
        page.says(event + 1);
        sent.push(event + 1);
    }
    page.run(100);
    heard.extend(game.try_iter());

    let events = |heard: &[DeviceEvent<u32>]| -> Vec<u32> {
        let mut events: Vec<u32> = heard
            .iter()
            .filter_map(|heard| match heard {
                DeviceEvent::Event { event, .. } => Some(*event),
                _ => None,
            })
            .collect();
        events.sort_unstable();
        events
    };
    sent.sort_unstable();
    assert_eq!(events(&heard), sent, "each event once");
    let mut echoes = page.events.clone();
    echoes.sort_unstable();
    assert_eq!(echoes, sent, "each echo once");
    assert_eq!(page.resets, 0, "the host never lost anything");
    assert!(
        !page.statuses.contains(&Status::Replaced),
        "{:?}",
        page.statuses
    );
    let moves: Vec<&DeviceEvent<u32>> = heard
        .iter()
        .filter(|heard| !matches!(heard, DeviceEvent::Event { .. } | DeviceEvent::State { .. }))
        .collect();
    let [DeviceEvent::Arrived { device, .. }, comings @ ..] = &moves[..] else {
        panic!("{moves:?}");
    };
    let device = *device;
    let left = DeviceEvent::Left {
        device,
        why: Why::Closed,
    };
    let back = DeviceEvent::Back { device };
    assert_eq!(comings, [&left, &back].repeat(3));
}

/// `slow_close` the other way round. The page stops running long enough for
/// the host to drop its link. The FIN stays in the relay, but WebSocket's close
/// frame is data and goes ahead of it: that frame is the end, not the FIN that
/// comes 30 s later, and the page comes back as itself.
#[test]
fn a_page_does_not_wait_for_a_close_that_is_slow_to_come() {
    let timing = Timing {
        heartbeat: 50,
        silent: 200,
        absent: 500,
        ..Timing::default()
    };
    let (host, game) = serve("half", timing);
    let relay = Relay::in_front_of(host);
    relay.slow_close(Duration::from_secs(30));
    let mut page = Page::open(relay.port, 6, Clock::start(), timing);
    page.says(1);
    let device = match &sorted(&game).0[..] {
        [DeviceEvent::Arrived { device, .. }] => *device,
        moves => panic!("{moves:?}"),
    };

    thread::sleep(Duration::from_millis(700));
    assert!(matches!(
        game.recv_timeout(Duration::from_secs(1)),
        Ok(DeviceEvent::Left {
            why: Why::Silent,
            ..
        })
    ));
    page.says(2);
    assert_eq!(page.ends, [ErrorKind::UnexpectedEof], "ended by the frame");
    assert_eq!(page.resets, 0);
    assert_eq!(sorted(&game), (vec![DeviceEvent::Back { device }], vec![2]));
}
