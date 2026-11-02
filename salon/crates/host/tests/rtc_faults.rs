//! WebRTC when a peer breaks its dependencies, end to end: a real server, real
//! UDP and HTTP on this computer, a real `Client` over a real DataChannel. The
//! pair is str0m, like the host: this says nothing of what a browser sends,
//! only of what the host does with what it hears.

#![expect(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "test: its helpers are outside the #[test] functions Clippy recognizes"
)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, Host, Kind, Limits, Ms, Timing, Token,
    route,
};
use salon_host::{Clock, Game, Options};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc, RtcConfig};

const VERSION: u32 = 1;
/// A peer that has not done it by then will not.
const PATIENCE_MS: u64 = 8_000;

/// Says each event back.
struct Echo;

impl Game for Echo {
    type Up = u32;
    type Down = u32;

    fn on(&mut self, _: Ms, event: DeviceEvent<u32>, host: &mut Host<u32, u32>) {
        if let DeviceEvent::Event { device, event, .. } = &event {
            host.send(*device, event).unwrap();
        }
    }
}

/// Serves the echo for the rest of the test run, and gives its port.
fn serve(name: &str) -> u16 {
    // Free on TCP and on UDP, which salon opens with the same number.
    let port = loop {
        let tcp = TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            break port;
        }
    };
    let dir = std::env::temp_dir().join(format!("salon-rtc-faults-{name}-{port}"));
    let mut options = Options::new(port, dir.clone(), VERSION, "ready");
    options.data = Some(dir);
    options.tick = Duration::from_millis(5);
    thread::spawn(move || salon_host::run(options, Echo));
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        thread::sleep(Duration::from_millis(5));
    }
    port
}

/// POSTs `body` as an offer: the HTTP status and what came after the head.
fn post(port: u16, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let head = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        route::OFFER,
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let end = response.windows(4).position(|end| end == b"\r\n\r\n");
    let head = String::from_utf8_lossy(&response[..end.unwrap_or(response.len())]);
    let status = head.split_whitespace().nth(1).unwrap_or_default();
    let body = end.map_or_else(Vec::new, |end| response[end + 4..].to_vec());
    (status.parse().unwrap_or(0), body)
}

/// The epoch and the bytes of each DTLS record of `datagram`.
fn records(datagram: &[u8]) -> Vec<(u16, &[u8])> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(header) = datagram.get(at..at + 13) {
        let epoch = u16::from_be_bytes([header[3], header[4]]);
        let end = at + 13 + usize::from(u16::from_be_bytes([header[11], header[12]]));
        let Some(record) = datagram.get(at..end) else {
            break;
        };
        found.push((epoch, record));
        at = end;
    }
    found
}

/// What a peer does with its handshake.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Conduct {
    Honest,
    /// At its first encrypted record, sends that record alone, twice, and
    /// stops talking: dimpl 0.7.6's assertion, without a token.
    Forges,
    Forged,
}

/// A page, as far as the host can tell: a real `Client`, a real DataChannel.
struct Peer {
    rtc: Rtc,
    channel: Option<ChannelId>,
    socket: UdpSocket,
    address: SocketAddr,
    timeout: Instant,
    client: Client<u32, u32>,
    clock: Clock,
    events: Vec<u32>,
    conduct: Conduct,
}

impl Peer {
    /// Offers a channel to the host; what the host answers must be an answer.
    fn dial(port: u16, token: u8, clock: Clock, conduct: Conduct) -> Peer {
        let socket = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        socket.set_nonblocking(true).unwrap();
        let address = socket.local_addr().unwrap();
        let mut rtc: Rtc = RtcConfig::new().build(Clock::instant());
        rtc.add_local_candidate(Candidate::host(address, "udp").unwrap());
        let mut change = rtc.sdp_api();
        change.add_channel_with_config(ChannelConfig {
            label: "salon".to_owned(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..ChannelConfig::default()
        });
        let (offer, pending) = change.apply().unwrap();
        let body = serde_json::to_vec::<SdpOffer>(&offer).unwrap();
        let (status, answer) = post(port, &body);
        assert_eq!(status, 200, "the host refused an offer");
        let answer: SdpAnswer = serde_json::from_slice(&answer).unwrap();
        rtc.sdp_api().accept_answer(pending, answer).unwrap();
        let config = ClientConfig {
            game: VERSION,
            kind: Kind::Phone,
            token: Token([token; 16]),
            name: None,
            tv_key: None,
            seed: u64::from(token),
            timing: Timing::default(),
            limits: Limits::default(),
        };
        Peer {
            rtc,
            channel: None,
            socket,
            address,
            timeout: Clock::instant(),
            client: Client::new(clock.now(), config),
            clock,
            events: Vec::new(),
            conduct,
        }
    }

    fn transmit(&mut self, destination: SocketAddr, datagram: &[u8]) {
        let first = records(datagram)
            .into_iter()
            .find(|&(epoch, _)| epoch == 1)
            .filter(|_| (20..64).contains(&datagram[0]));
        match (self.conduct, first) {
            (Conduct::Forged, _) => {}
            (Conduct::Forges, Some((_, record))) => {
                self.socket.send_to(record, destination).unwrap();
                self.socket.send_to(record, destination).unwrap();
                self.conduct = Conduct::Forged;
            }
            (Conduct::Honest | Conduct::Forges, _) => {
                self.socket.send_to(datagram, destination).unwrap();
            }
        }
    }

    /// Everything str0m wants done now.
    fn drain(&mut self) {
        let now = self.clock.now();
        loop {
            match self.rtc.poll_output().unwrap() {
                Output::Timeout(at) => {
                    self.timeout = at;
                    return;
                }
                Output::Transmit(transmit) => {
                    self.transmit(transmit.destination, &transmit.contents);
                }
                Output::Event(Event::ChannelOpen(id, _)) => {
                    self.channel = Some(id);
                    self.client.opened(now, Caps::DATAGRAM);
                }
                Output::Event(Event::ChannelData(data)) => self.client.received(now, &data.data),
                Output::Event(_) => {}
            }
        }
    }

    fn pump(&mut self) {
        self.drain();
        let now = self.clock.now();
        while let Some(output) = self.client.poll(now) {
            match output {
                ClientOutput::Transmit(bytes) => {
                    if let Some(mut channel) = self.channel.and_then(|id| self.rtc.channel(id)) {
                        channel.write(true, &bytes).unwrap();
                    }
                }
                ClientOutput::Event(event) => self.events.push(event),
                _ => {}
            }
        }
        self.drain();
        let mut buffer = [0; 2048];
        while let Ok((read, source)) = self.socket.recv_from(&mut buffer) {
            let input = Input::Receive(
                Clock::instant(),
                Receive {
                    proto: Protocol::Udp,
                    source,
                    destination: self.address,
                    contents: buffer[..read].try_into().unwrap(),
                },
            );
            self.rtc.handle_input(input).unwrap();
            self.drain();
        }
        let now = Clock::instant();
        if self.timeout <= now {
            self.rtc.handle_input(Input::Timeout(now)).unwrap();
        }
    }
}

/// Pumps every peer until `done`, at most [`PATIENCE_MS`].
fn until(peers: &mut [&mut Peer], mut done: impl FnMut(&[&mut Peer]) -> bool) -> bool {
    let end = Clock::instant() + Duration::from_millis(PATIENCE_MS);
    while Clock::instant() < end {
        for peer in peers.iter_mut() {
            peer.pump();
        }
        if done(peers) {
            return true;
        }
        thread::sleep(Duration::from_millis(1));
    }
    false
}

/// `event` goes to the host and comes back over the phone's DataChannel, while
/// `others` go on.
fn says(phone: &mut Peer, others: &mut [&mut Peer], event: u32) {
    phone.client.send(&event).unwrap();
    let mut peers: Vec<&mut Peer> = others.iter_mut().map(|peer| &mut **peer).collect();
    peers.insert(0, phone);
    assert!(
        until(&mut peers, |peers| peers[0].events.contains(&event)),
        "no echo of {event}"
    );
}

/// E1: an unauthenticated page, with an offer and two equal records, must cost
/// its own link. dimpl 0.7.6 panics on them, and that thread serves every page:
/// it took WebRTC away from the whole room, and every later offer got a 503.
#[test]
fn a_panic_in_a_dependency_costs_one_link() {
    let port = serve("panic");
    let clock = Clock::start();
    let mut phone = Peer::dial(port, 1, clock, Conduct::Honest);
    says(&mut phone, &mut [], 1);

    let mut forger = Peer::dial(port, 2, clock, Conduct::Forges);
    assert!(
        until(&mut [&mut forger, &mut phone], |peers| {
            peers[0].conduct == Conduct::Forged
        }),
        "the forger never reached its first encrypted record"
    );
    // The two records are on their way: time for the host to read them.
    let read = Clock::instant() + Duration::from_millis(200);
    until(&mut [&mut forger, &mut phone], |_| Clock::instant() >= read);

    says(&mut phone, &mut [], 2);
    let mut late = Peer::dial(port, 3, clock, Conduct::Honest);
    says(&mut late, &mut [&mut phone], 3);
    assert!(late.channel.is_some(), "a new phone must get WebRTC");
    says(&mut phone, &mut [&mut late], 4);
}
