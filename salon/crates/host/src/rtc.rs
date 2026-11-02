//! WebRTC links: every page's DataChannel, on one UDP port, driven by str0m
//! in ICE-lite. The host announces the address the page's offer came to, the
//! one it would give anyway, and its loopback one (a page of this very computer
//! reaches it without the firewall); it learns each phone's from its first
//! packet (Safari hides its own behind a `.local` name). The cryptography is
//! salon-crypto's, in Rust only.
//!
//! str0m wants every input followed by draining its outputs before the next
//! input (`str0m/src/lib.rs`, "single-mutation invariant"): [`drive`] does
//! that after each one.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use salon_core::{Caps, LinkId};
use salon_crypto::DtlsCert;
use str0m::change::SdpOffer;
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{
    Candidate, DATAGRAM_MTU_WARN, Event, IceConnectionState, Input, Output, Rtc, RtcConfig,
};

use crate::clock::Clock;
use crate::room::inbox::{Refused, Sender};
use crate::room::{Net, Outlet, RtcOut};

/// A page that has not opened its channel by then is dropped: str0m has no
/// timeout of its own before it hears from the other side.
const HANDSHAKE: Duration =
    Duration::from_secs(salon_core::transport::RTC_NEGOTIATION_SECONDS as u64);
/// Longest a read waits before the thread checks for packets to send: one
/// thread serves every page, so waking a thousand times a second costs little
/// and keeps outgoing packets from waiting.
const POLL: Duration = Duration::from_millis(1);
/// Between two starts of the thread: what panics out of [`run`] is no packet's
/// fault, so it may well panic again at once, and this keeps that from eating
/// a core. Short enough that a page's next offer finds the thread back.
const RESTART: Duration = Duration::from_millis(100);
/// str0m's datagram size, sized so that the largest packet `Caps::DATAGRAM`
/// allows leaves in one datagram: str0m takes 48 octets of DTLS and 40 of SCTP
/// off it, and its default of 1 150 cut every packet over 1 062 in two. What
/// leaves is 1 165 octets, 1 193 with IP and UDP: under the 1 280 that IPv6
/// promises any path.
const MTU: usize = Caps::DATAGRAM.max_packet.saturating_add(48 + 40);

pub struct Signal {
    pub link: LinkId,
    pub rtc: Rtc,
    /// The address the offer came to: see [`Entry::reached`].
    pub reached: SocketAddr,
}

/// The loopback address on the same port.
fn loopback(address: SocketAddr) -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), address.port())
}

/// What a page is told to try, without repeats. `reached` first: the phone
/// has just spoken to this address over TCP, whatever the computer's default
/// route says, on a Mac with two interfaces, or since DHCP moved it. `announced`
/// is the guess made at start, or the address `SALON_PUBLIC_URL` names, which
/// the socket cannot know behind a forwarded port. And the loopback address,
/// for pages of this very computer. A phone ignores a candidate it cannot reach.
fn candidates(reached: SocketAddr, announced: SocketAddr) -> Vec<SocketAddr> {
    let mut addresses = vec![reached];
    for address in [announced, loopback(reached)] {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses
}

/// Answers a page's offer (JSON, as `RTCSessionDescription`) with a new
/// connection in ICE-lite, with the certificate made at start.
pub fn answer(
    offer: &[u8],
    reached: SocketAddr,
    announced: SocketAddr,
    cert: &DtlsCert,
) -> Result<(Rtc, Vec<u8>), String> {
    let offer: SdpOffer =
        serde_json::from_slice(offer).map_err(|error| format!("bad offer: {error}"))?;
    let mut rtc = RtcConfig::new()
        .set_ice_lite(true)
        .set_dtls_cert(cert.clone())
        .set_mtu(MTU..=DATAGRAM_MTU_WARN)
        .build(Clock::instant());
    for candidate in candidates(reached, announced) {
        let candidate = Candidate::host(candidate, "udp").map_err(|error| error.to_string())?;
        rtc.add_local_candidate(candidate);
    }
    let answer = rtc
        .sdp_api()
        .accept_offer(offer)
        .map_err(|error| format!("offer refused: {error}"))?;
    let body = serde_json::to_vec(&answer).map_err(|error| error.to_string())?;
    Ok((rtc, body))
}

struct Entry {
    rtc: Rtc,
    channel: Option<ChannelId>,
    created: Instant,
    timeout: Option<Instant>,
    /// The page is on this very computer: its packets come from loopback.
    local: bool,
    /// A dependency panicked on this link: its state means nothing any more.
    broken: bool,
    /// The address the offer came to: the one its packets come to too, since
    /// str0m drops a check that is not for one of its candidates, and the
    /// socket listens on every interface without saying which one heard.
    reached: SocketAddr,
}

impl Entry {
    /// Runs `work` on this link. One page must never cost more than its own
    /// link, whatever it sends: dimpl 0.7.6 panics on two equal records from a
    /// peer without a token (`recherche/transport-webrtc.toml`, T1), and
    /// every page shares this thread. `None` when the link is broken, which
    /// nothing touches again.
    fn guarded<T>(&mut self, link: LinkId, work: impl FnOnce(&mut Entry) -> T) -> Option<T> {
        if self.broken {
            return None;
        }
        let outcome = contained(|| work(self));
        if outcome.is_none() {
            self.broken = true;
            crate::say(format_args!(
                "salon: WebRTC link {} panicked: it is dropped, the others go on",
                link.0
            ));
        }
        outcome
    }
}

/// `None` if `work` panicked. Needs unwinding: under `panic = "abort"`, which
/// no profile of salon, of the model or of a game sets, the process would just
/// stop. The server says so when it starts.
fn contained<T>(work: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(work)).ok()
}

/// The way to the room from the one thread that serves every page: it never
/// waits there, or a full inbox would stop every page's packets and handshakes.
/// A packet that does not fit is lost, like on any network, and salon sends it
/// again. What the room must hear waits here, in order, and a packet that
/// comes behind it is lost too: nothing overtakes the opening of its link.
pub struct ToRoom {
    net: Sender<Net>,
    waiting: RefCell<VecDeque<Net>>,
}

impl ToRoom {
    pub fn new(net: Sender<Net>) -> ToRoom {
        ToRoom {
            net,
            waiting: RefCell::new(VecDeque::new()),
        }
    }

    fn packet(&self, link: LinkId, bytes: Vec<u8>) {
        if self.flush() {
            // Full: this packet is lost, and salon sends it again.
            let _: Result<(), Refused<Net>> = self.net.try_send(Net::Received { link, bytes });
        }
    }

    fn tell(&self, net: Net) {
        self.waiting.borrow_mut().push_back(net);
        self.flush();
    }

    /// Hands over what waits. Says whether nothing waits any more.
    fn flush(&self) -> bool {
        let mut waiting = self.waiting.borrow_mut();
        while let Some(net) = waiting.pop_front() {
            match self.net.try_send(net) {
                Ok(()) | Err(Refused::Gone) => {}
                Err(Refused::Full(net)) => {
                    waiting.push_front(net);
                    return false;
                }
            }
        }
        true
    }
}

pub struct Rtcs {
    pub socket: UdpSocket,
    pub signals: Receiver<Signal>,
    pub outgoing: Receiver<RtcOut>,
    pub outlet: SyncSender<RtcOut>,
    pub net: ToRoom,
}

type Entries = BTreeMap<LinkId, Entry>;

impl Rtcs {
    /// Starts the thread that serves every page. What each link does is
    /// guarded by [`Entry::guarded`]; a panic that still comes out of [`run`]
    /// costs the links of this start, which the room hears of, and not WebRTC
    /// itself: the thread starts again, and the pages come back to it.
    pub fn start(self) -> Result<(), String> {
        if cfg!(panic = "abort") {
            crate::say(format_args!(
                "salon: this build aborts on panic: a peer that makes a dependency panic stops the game"
            ));
        }
        thread::Builder::new()
            .name("salon-rtc".into())
            .spawn(move || keep_up(&self, run))
            .map(drop)
            .map_err(|error| format!("cannot start WebRTC: {error}"))
    }
}

fn keep_up(rtcs: &Rtcs, serving: impl Fn(&Rtcs, &mut Entries)) {
    loop {
        let mut entries = Entries::new();
        if contained(|| serving(rtcs, &mut entries)).is_some() {
            return;
        }
        crate::say(format_args!(
            "salon: the WebRTC thread panicked: its links are closed, and it starts again"
        ));
        for (link, entry) in entries {
            if entry.channel.is_some() {
                rtcs.net.tell(Net::Closed { link });
            }
            // Nothing can be asked of it, and dropping it is still its code.
            let _: Option<()> = contained(|| drop(entry));
        }
        thread::sleep(RESTART);
    }
}

fn run(rtcs: &Rtcs, entries: &mut Entries) {
    if rtcs.socket.set_read_timeout(Some(POLL)).is_err() {
        crate::say(format_args!(
            "salon: cannot set the UDP socket's timeout; WebRTC is off"
        ));
        return;
    }
    let mut buffer = vec![0_u8; 2048];
    loop {
        rtcs.net.flush();
        loop {
            match rtcs.signals.try_recv() {
                Ok(Signal { link, rtc, reached }) => {
                    let mut entry = Entry {
                        rtc,
                        reached,
                        channel: None,
                        created: Clock::instant(),
                        timeout: None,
                        local: false,
                        broken: false,
                    };
                    let _: Option<()> = entry.guarded(link, |entry| drive(rtcs, link, entry));
                    entries.insert(link, entry);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        while let Ok(out) = rtcs.outgoing.try_recv() {
            match out {
                RtcOut::Packet(link, bytes) => {
                    if let Some(entry) = entries.get_mut(&link)
                        && let Some(id) = entry.channel
                    {
                        let _: Option<()> = entry.guarded(link, |entry| {
                            if let Some(mut channel) = entry.rtc.channel(id) {
                                // `false`: str0m's buffer is full; the packet is
                                // lost and salon sends it again.
                                let _: Result<bool, str0m::RtcError> = channel.write(true, &bytes);
                            }
                            drive(rtcs, link, entry);
                        });
                    }
                }
                RtcOut::Close(link) => {
                    if let Some(mut entry) = entries.remove(&link) {
                        let _: Option<()> = entry.guarded(link, |entry| entry.rtc.disconnect());
                    }
                }
            }
        }
        receive(rtcs, entries, &mut buffer);
        let now = Clock::instant();
        for (&link, entry) in entries.iter_mut() {
            if entry.timeout.is_some_and(|at| at <= now) {
                let _: Option<()> = entry.guarded(link, |entry| {
                    if entry.rtc.handle_input(Input::Timeout(now)).is_err() {
                        entry.rtc.disconnect();
                    }
                    drive(rtcs, link, entry);
                });
            }
        }
        entries.retain(|&link, entry| {
            let stalled = entry.channel.is_none() && now.duration_since(entry.created) > HANDSHAKE;
            let alive = entry.guarded(link, |entry| entry.rtc.is_alive());
            let keep = alive == Some(true) && !stalled;
            if !keep && std::env::var_os("SALON_LOG").is_some() {
                crate::say(format_args!(
                    "salon: WebRTC link {} dropped (stalled: {stalled})",
                    link.0
                ));
            }
            if !keep && entry.channel.is_some() {
                rtcs.net.tell(Net::Closed { link });
            }
            keep
        });
    }
}

/// Reads one datagram, if any, and gives it to the connection it belongs to.
fn receive(rtcs: &Rtcs, entries: &mut Entries, buffer: &mut [u8]) {
    let (size, source) = match rtcs.socket.recv_from(buffer) {
        Ok(read) => read,
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return,
        Err(error) => {
            crate::say(format_args!("salon: UDP: {error}"));
            return;
        }
    };
    if let Some(contents) = buffer.get(..size) {
        deliver(rtcs, entries, source, contents);
    }
}

/// Where a packet from `source` came to, for a link reached at `reached`. The
/// socket listens on every interface: a packet from this computer came to the
/// loopback candidate, and any other to the address the offer came to.
fn destination(reached: SocketAddr, source: SocketAddr) -> SocketAddr {
    if source.ip().is_loopback() {
        loopback(reached)
    } else {
        reached
    }
}

/// Gives a datagram to the connection it belongs to.
fn deliver(rtcs: &Rtcs, entries: &mut Entries, source: SocketAddr, contents: &[u8]) {
    // A datagram nobody can read yet belongs to no link, so no link pays for it.
    let Some(Ok(contents)) = contained(|| contents.try_into()) else {
        return;
    };
    let mut input = Input::Receive(
        Clock::instant(),
        Receive {
            proto: Protocol::Udp,
            source,
            // `accepts` does not read it, and the link is not known yet.
            destination: source,
            contents,
        },
    );
    let found = entries.iter_mut().find_map(|(&link, entry)| {
        (entry.guarded(link, |entry| entry.rtc.accepts(&input)) == Some(true))
            .then_some((link, entry))
    });
    let Some((link, entry)) = found else {
        return;
    };
    if let Input::Receive(_, receive) = &mut input {
        receive.destination = destination(entry.reached, source);
    }
    entry.local = source.ip().is_loopback();
    let _: Option<()> = entry.guarded(link, |entry| {
        if entry.rtc.handle_input(input).is_ok() {
            drive(rtcs, link, entry);
        } else {
            entry.rtc.disconnect();
        }
    });
}

/// Drains `entry`'s outputs: packets to send, events for the room, and the
/// next timeout.
fn drive(rtcs: &Rtcs, link: LinkId, entry: &mut Entry) {
    loop {
        match entry.rtc.poll_output() {
            Ok(Output::Timeout(at)) => {
                entry.timeout = Some(at);
                return;
            }
            Ok(Output::Transmit(transmit)) => {
                // UDP: a datagram that does not leave is a lost packet.
                let _: std::io::Result<usize> = rtcs
                    .socket
                    .send_to(&transmit.contents, transmit.destination);
            }
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "str0m::Event is #[non_exhaustive] and wide: a data-only link ignores every other event"
            )]
            Ok(Output::Event(event)) => match event {
                Event::ChannelOpen(id, _) if entry.channel.is_none() => {
                    entry.channel = Some(id);
                    let opened = Net::Opened {
                        link,
                        // Only a link from this computer may be the TV.
                        caps: if entry.local {
                            Caps::DATAGRAM.local()
                        } else {
                            Caps::DATAGRAM
                        },
                        outlet: Outlet::Rtc(rtcs.outlet.clone()),
                    };
                    rtcs.net.tell(opened);
                }
                Event::ChannelData(data) if Some(data.id) == entry.channel => {
                    rtcs.net.packet(link, data.data);
                }
                Event::ChannelClose(id) if Some(id) == entry.channel => entry.rtc.disconnect(),
                Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                    entry.rtc.disconnect();
                }
                _ => {}
            },
            Err(_) => {
                entry.rtc.disconnect();
                return;
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;

    use str0m::change::SdpAnswer;
    use str0m::channel::{ChannelConfig, Reliability};

    use super::*;
    use crate::room::inbox::{self, channel};

    const PAGE: &str = "192.0.2.50:5000";
    const HOST: &str = "192.0.2.10:8090";

    /// A page and the host's end of its link, as [`answer`] makes it, over a
    /// memory and in virtual time.
    struct Pair {
        page: Rtc,
        host: Rtc,
        now: Instant,
        page_channel: Option<ChannelId>,
        host_channel: Option<ChannelId>,
        /// The size of each datagram the host sent.
        down: Vec<usize>,
        /// What the page read on its channel.
        heard: Vec<Vec<u8>>,
    }

    impl Pair {
        /// Offer, answer, ICE, DTLS, SCTP: both ends of the channel are open.
        fn opened() -> Pair {
            let mut pair = Pair::answered();
            pair.run_until(|pair| pair.page_channel.is_some() && pair.host_channel.is_some());
            pair.down.clear();
            pair
        }

        /// Offer and answer: ICE has not started.
        fn answered() -> Pair {
            salon_crypto::provider().install_process_default();
            let now = Clock::instant();
            let mut page: Rtc = RtcConfig::new().build(now);
            let at: SocketAddr = PAGE.parse().unwrap();
            page.add_local_candidate(Candidate::host(at, "udp").unwrap());
            let mut change = page.sdp_api();
            change.add_channel_with_config(ChannelConfig {
                label: "salon".to_owned(),
                ordered: false,
                reliability: Reliability::MaxRetransmits { retransmits: 0 },
                ..ChannelConfig::default()
            });
            let (offer, pending) = change.apply().unwrap();
            let offer = serde_json::to_vec::<SdpOffer>(&offer).unwrap();
            let cert = salon_crypto::dtls_cert().unwrap();
            let host_address = HOST.parse().unwrap();
            let (host, answer_body) = answer(&offer, host_address, host_address, &cert).unwrap();
            let answered: SdpAnswer = serde_json::from_slice(&answer_body).unwrap();
            page.sdp_api().accept_answer(pending, answered).unwrap();
            Pair {
                page,
                host,
                now,
                page_channel: None,
                host_channel: None,
                down: Vec::new(),
                heard: Vec::new(),
            }
        }

        /// Drains both ends and delivers everything, once.
        fn exchange(&mut self) {
            let mut wire: VecDeque<(SocketAddr, SocketAddr, Vec<u8>)> = VecDeque::new();
            loop {
                self.drain_page(&mut wire);
                self.drain_host(&mut wire);
                let Some((source, destination, contents)) = wire.pop_front() else {
                    return;
                };
                let input = Input::Receive(
                    self.now,
                    Receive {
                        proto: Protocol::Udp,
                        source,
                        destination,
                        contents: contents.as_slice().try_into().unwrap(),
                    },
                );
                if destination == HOST.parse().unwrap() {
                    self.host.handle_input(input).unwrap();
                } else {
                    self.page.handle_input(input).unwrap();
                }
            }
        }

        fn drain_page(&mut self, wire: &mut VecDeque<(SocketAddr, SocketAddr, Vec<u8>)>) {
            loop {
                match self.page.poll_output().unwrap() {
                    Output::Timeout(_) => return,
                    Output::Transmit(sent) => {
                        wire.push_back((sent.source, sent.destination, sent.contents.to_vec()));
                    }
                    Output::Event(Event::ChannelOpen(id, _)) => self.page_channel = Some(id),
                    Output::Event(Event::ChannelData(data)) => self.heard.push(data.data),
                    Output::Event(_) => {}
                }
            }
        }

        fn drain_host(&mut self, wire: &mut VecDeque<(SocketAddr, SocketAddr, Vec<u8>)>) {
            loop {
                match self.host.poll_output().unwrap() {
                    Output::Timeout(_) => return,
                    Output::Transmit(sent) => {
                        self.down.push(sent.contents.len());
                        wire.push_back((sent.source, sent.destination, sent.contents.to_vec()));
                    }
                    Output::Event(Event::ChannelOpen(id, _)) => self.host_channel = Some(id),
                    Output::Event(_) => {}
                }
            }
        }

        fn run_until(&mut self, done: impl Fn(&Pair) -> bool) {
            let end = self.now + Duration::from_secs(30);
            while self.now < end {
                self.exchange();
                if done(self) {
                    return;
                }
                self.now += Duration::from_millis(10);
                self.host.handle_input(Input::Timeout(self.now)).unwrap();
                self.page.handle_input(Input::Timeout(self.now)).unwrap();
            }
            panic!("not done within 30 virtual seconds");
        }
    }

    /// The thread's way to everything, on a port nobody talks to.
    fn door(room: usize) -> (Rtcs, inbox::Receiver<Net>) {
        let (net, inbox) = channel(room).unwrap();
        let (outlet, outgoing) = sync_channel(1);
        let (_, signals) = sync_channel(1);
        let rtcs = Rtcs {
            socket: UdpSocket::bind(("127.0.0.1", 0)).unwrap(),
            signals,
            outgoing,
            outlet,
            net: ToRoom::new(net),
        };
        (rtcs, inbox)
    }

    fn entry_of(pair: Pair) -> Entry {
        Entry {
            rtc: pair.host,
            channel: pair.host_channel,
            created: pair.now,
            timeout: None,
            local: false,
            broken: false,
            reached: HOST.parse().unwrap(),
        }
    }

    /// What panics out of the loop, past every guard, costs the links of this
    /// start: the room hears of them, and the thread is there for the next
    /// offer. Dead, it left every later offer a 503.
    #[test]
    fn the_thread_starts_again_and_the_room_hears_what_it_lost() {
        let (rtcs, inbox) = door(4);
        let pair = Cell::new(Some(Pair::opened()));
        let starts = Cell::new(0_u32);
        keep_up(&rtcs, |_, entries| {
            starts.set(starts.get() + 1);
            if let Some(pair) = pair.take() {
                entries.insert(LinkId(7), entry_of(pair));
                std::panic::panic_any("outside every guard");
            }
        });
        assert_eq!(starts.get(), 2);
        let read = inbox.recv_timeout(Duration::ZERO).unwrap();
        assert!(matches!(read, Net::Closed { link } if link == LinkId(7)));
    }

    /// A link that panicked is not asked anything again: its state means
    /// nothing, and a second panic there would prove nothing either.
    #[test]
    fn a_broken_link_is_never_touched_again() {
        let mut entry = entry_of(Pair::opened());
        let link = LinkId(1);
        assert_eq!(entry.guarded(link, |_| 1), Some(1));
        let panicked: Option<u8> = entry.guarded(link, |_| std::panic::panic_any("a dependency"));
        assert_eq!(panicked, None);
        let touched = Cell::new(false);
        assert_eq!(entry.guarded(link, |_| touched.set(true)), None);
        assert!(!touched.get());
    }

    /// What a page reads in the answer: the address of each candidate.
    fn announced(reached: &str, guess: &str) -> Vec<String> {
        let offer = RtcConfig::new().build(Clock::instant());
        let mut offer = offer;
        let mut change = offer.sdp_api();
        change.add_channel_with_config(ChannelConfig {
            label: "salon".to_owned(),
            ..ChannelConfig::default()
        });
        let (offer, _) = change.apply().unwrap();
        let offer = serde_json::to_vec::<SdpOffer>(&offer).unwrap();
        let cert = salon_crypto::dtls_cert().unwrap();
        let reached = reached.parse().unwrap();
        let (_, body) = answer(&offer, reached, guess.parse().unwrap(), &cert).unwrap();
        let answer: SdpAnswer = serde_json::from_slice(&body).unwrap();
        answer
            .to_sdp_string()
            .lines()
            .filter(|line| line.starts_with("a=candidate:"))
            .filter_map(|line| line.split_whitespace().nth(4).map(str::to_owned))
            .collect()
    }

    /// T4: `Caps::DATAGRAM` promises one network frame, since losing a
    /// fragment loses the message on a channel that does not retransmit. The
    /// largest packet it allows went out in two.
    #[test]
    fn the_largest_datagram_packet_is_one_datagram() {
        let mut pair = Pair::opened();
        pair.exchange();
        pair.down.clear();
        let packet = vec![7; Caps::DATAGRAM.max_packet];
        let id = pair.host_channel.unwrap();
        let mut channel = pair.host.channel(id).unwrap();
        assert!(channel.write(true, &packet).unwrap());
        pair.exchange();
        assert_eq!(pair.down.len(), 1, "{:?}", pair.down);
        assert_eq!(pair.heard, [packet]);
        // Under what a Wi-Fi carries, and what IPv6 promises any path, with
        // the 28 octets of IP and UDP.
        assert!(pair.down[0] + 28 <= 1280, "{:?}", pair.down);
    }

    /// T3: a phone has just reached this address, the one the host would have
    /// guessed may lead nowhere, and a page must not pay the window for it.
    #[test]
    fn the_address_an_offer_came_to_is_announced() {
        salon_crypto::provider().install_process_default();
        assert_eq!(
            announced("192.168.1.10:8090", "10.0.0.2:8090"),
            ["192.168.1.10", "10.0.0.2", "127.0.0.1"]
        );
        // Nothing twice: the same address, and a page of this very computer.
        assert_eq!(
            announced("192.168.1.10:8090", "192.168.1.10:8090"),
            ["192.168.1.10", "127.0.0.1"]
        );
        assert_eq!(
            announced("127.0.0.1:8090", "192.168.1.10:8090"),
            ["127.0.0.1", "192.168.1.10"]
        );
    }

    /// T3: str0m drops an ICE check that is not for one of its candidates, so
    /// each link hears on the address of its own offer. A link reached at a
    /// second address used to be told it heard on the first one, and its
    /// page waited out the window.
    #[test]
    fn a_link_hears_on_the_address_of_its_offer() {
        let (rtcs, _inbox) = door(4);
        let mut pair = Pair::answered();
        let mut wire = VecDeque::new();
        while wire.is_empty() {
            pair.now += Duration::from_millis(10);
            pair.page.handle_input(Input::Timeout(pair.now)).unwrap();
            pair.drain_page(&mut wire);
        }
        let (source, _, check) = wire.pop_front().unwrap();
        let mut entries = Entries::new();
        entries.insert(LinkId(1), entry_of(pair));
        deliver(&rtcs, &mut entries, source, &check);
        // A check str0m took makes the source a known one: a DTLS packet from
        // there belongs to this link now.
        let dtls: &[u8] = &[22, 0xfe, 0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let probe = Input::Receive(
            Clock::instant(),
            Receive {
                proto: Protocol::Udp,
                source,
                destination: HOST.parse().unwrap(),
                contents: dtls.try_into().unwrap(),
            },
        );
        assert!(entries.get(&LinkId(1)).unwrap().rtc.accepts(&probe));
    }

    #[test]
    fn a_packet_from_this_computer_came_to_the_loopback_candidate() {
        let reached: SocketAddr = HOST.parse().unwrap();
        let phone: SocketAddr = PAGE.parse().unwrap();
        assert_eq!(destination(reached, phone), reached);
        let local: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert_eq!(destination(reached, local), loopback(reached));
    }

    fn opened(link: LinkId) -> Net {
        let (outlet, _) = sync_channel(1);
        Net::Opened {
            link,
            caps: Caps::DATAGRAM,
            outlet: Outlet::Rtc(outlet),
        }
    }

    /// The thread of every page does not wait for a full inbox, and what the
    /// room must hear still gets there, before the packets that follow it.
    #[test]
    fn a_full_inbox_stops_nothing_and_loses_only_packets() {
        let (net, inbox) = channel(1).unwrap();
        let door = ToRoom::new(net);
        let (first, second) = (LinkId(1), LinkId(2));
        door.packet(first, vec![1]);
        // Lost, not waited for: this call would never come back otherwise.
        door.packet(first, vec![2]);
        door.tell(opened(second));
        door.tell(Net::Closed { link: first });
        door.packet(second, vec![3]);
        let read = || inbox.recv_timeout(Duration::ZERO).unwrap();
        assert!(matches!(read(), Net::Received { bytes, .. } if bytes == [1]));
        assert!(!door.flush(), "one of the two fits");
        assert!(matches!(read(), Net::Opened { link, .. } if link == second));
        assert!(door.flush());
        assert!(matches!(read(), Net::Closed { link } if link == first));
        door.packet(second, vec![4]);
        assert!(matches!(read(), Net::Received { bytes, .. } if bytes == [4]));
    }
}
