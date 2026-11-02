//! The handshake when the network or the peer is not what `handshake.rs`
//! assumes: every datagram lost, then doubled, one at a time, each way, and
//! records forged from the page's address. Two str0m in virtual time: what a
//! browser really sends is `tools/cert/rtc-negotiation.mjs`'s to say. These
//! tests pin what these versions of str0m, dimpl and sctp-proto do, so that
//! raising one of them is tried against the same faults.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] functions: a failed step stops the test with its message"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc, RtcConfig};

const PAGE: &str = "192.168.1.20:50000";
const HOST: &str = "192.168.1.10:8090";
/// `salon_core::transport::RTC_NEGOTIATION_SECONDS`: what a page gives a
/// negotiation before WebSocket. This crate does not depend on salon-core.
const WINDOW: Duration = Duration::from_secs(15);
/// First byte of a DTLS record, by RFC 9443's demultiplexing.
const DTLS: std::ops::Range<u8> = 20..64;

fn page_address() -> SocketAddr {
    PAGE.parse().expect("an address")
}

fn host_address() -> SocketAddr {
    HOST.parse().expect("an address")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Way {
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fault {
    Lost,
    Doubled,
}

/// The `nth` datagram that goes `way`, from 1.
#[derive(Clone, Copy, Debug)]
struct Plan {
    way: Way,
    nth: usize,
    fault: Fault,
}

/// What may be done to the page's datagrams on their way up.
type Mangle<'a> = &'a mut dyn FnMut(Vec<u8>) -> Vec<Vec<u8>>;

struct Side {
    rtc: Rtc,
    timeout: Instant,
    opened: Option<ChannelId>,
    received: Vec<Vec<u8>>,
}

impl Side {
    /// Drains until it asks for a timeout; `wire` takes its packets.
    fn drain(&mut self, way: Way, wire: &mut VecDeque<(Way, SocketAddr, SocketAddr, Vec<u8>)>) {
        loop {
            match self.rtc.poll_output().expect("str0m output") {
                Output::Timeout(at) => {
                    self.timeout = at;
                    return;
                }
                Output::Transmit(sent) => {
                    wire.push_back((way, sent.source, sent.destination, sent.contents.to_vec()));
                }
                Output::Event(Event::ChannelOpen(id, _)) => self.opened = Some(id),
                Output::Event(Event::ChannelData(data)) => self.received.push(data.data),
                Output::Event(_) => {}
            }
        }
    }
}

/// The two ends of what salon negotiates, over a memory and in virtual time.
struct Link {
    page: Side,
    host: Side,
    start: Instant,
    now: Instant,
    sent_up: usize,
    sent_down: usize,
}

impl Link {
    fn new() -> Link {
        salon_crypto::provider().install_process_default();
        #[expect(
            clippy::disallowed_methods,
            reason = "str0m wants a real Instant to start from; time is virtual after"
        )]
        let start = Instant::now();
        let cert = salon_crypto::dtls_cert().expect("a certificate");
        let side = |rtc| Side {
            rtc,
            timeout: start,
            opened: None,
            received: Vec::new(),
        };
        let mut page = side(RtcConfig::new().build(start));
        let mut host = side(
            RtcConfig::new()
                .set_ice_lite(true)
                .set_dtls_cert(cert)
                .build(start),
        );
        page.rtc
            .add_local_candidate(Candidate::host(page_address(), "udp").expect("a candidate"));
        host.rtc
            .add_local_candidate(Candidate::host(host_address(), "udp").expect("a candidate"));
        let mut change = page.rtc.sdp_api();
        change.add_channel_with_config(ChannelConfig {
            label: "salon".to_owned(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..ChannelConfig::default()
        });
        let (offer, pending) = change.apply().expect("an offer");
        let answer = host
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .expect("the host answers");
        page.rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("the page takes the answer");
        Link {
            page,
            host,
            start,
            now: start,
            sent_up: 0,
            sent_down: 0,
        }
    }

    fn side(&mut self, destination: SocketAddr) -> &mut Side {
        if destination == host_address() {
            &mut self.host
        } else {
            &mut self.page
        }
    }

    fn deliver(&mut self, source: SocketAddr, destination: SocketAddr, contents: &[u8]) {
        let input = Input::Receive(
            self.now,
            Receive {
                proto: Protocol::Udp,
                source,
                destination,
                contents: contents.try_into().expect("a datagram"),
            },
        );
        self.side(destination)
            .rtc
            .handle_input(input)
            .expect("str0m input");
    }

    /// Moves every datagram there is, faults included.
    fn exchange(&mut self, plan: Option<Plan>, mangle: Mangle<'_>) {
        let mut wire = VecDeque::new();
        loop {
            self.page.drain(Way::Up, &mut wire);
            self.host.drain(Way::Down, &mut wire);
            let Some((way, source, destination, contents)) = wire.pop_front() else {
                return;
            };
            let nth = match way {
                Way::Up => {
                    self.sent_up += 1;
                    self.sent_up
                }
                Way::Down => {
                    self.sent_down += 1;
                    self.sent_down
                }
            };
            let fault = plan
                .filter(|plan| plan.way == way && plan.nth == nth)
                .map(|plan| plan.fault);
            if fault == Some(Fault::Lost) {
                continue;
            }
            let copies = if fault == Some(Fault::Doubled) { 2 } else { 1 };
            let datagrams = if way == Way::Up {
                mangle(contents)
            } else {
                vec![contents]
            };
            for _ in 0..copies {
                for datagram in &datagrams {
                    self.deliver(source, destination, datagram);
                }
            }
        }
    }

    /// Runs until the channel is open on both sides: when, or `None` if it is
    /// not within [`WINDOW`].
    fn open(&mut self, plan: Option<Plan>, mangle: Mangle<'_>) -> Option<Duration> {
        while self.now - self.start <= WINDOW {
            self.exchange(plan, mangle);
            if self.page.opened.is_some() && self.host.opened.is_some() {
                return Some(self.now - self.start);
            }
            self.tick();
        }
        None
    }

    fn tick(&mut self) {
        self.now = self
            .page
            .timeout
            .min(self.host.timeout)
            .max(self.now + Duration::from_millis(1));
        for side in [&mut self.page, &mut self.host] {
            side.rtc
                .handle_input(Input::Timeout(self.now))
                .expect("str0m timeout");
        }
    }

    /// The page writes on its channel: the host hears it, so the link lives.
    fn speaks(&mut self, words: &[u8]) {
        let channel = self.page.opened.expect("an open channel");
        self.page
            .rtc
            .channel(channel)
            .expect("the page's channel")
            .write(true, words)
            .expect("the page writes");
        let goal = self.now + Duration::from_secs(1);
        while self.now < goal && !self.host.received.iter().any(|heard| heard == words) {
            self.exchange(None, &mut |datagram| vec![datagram]);
            self.tick();
        }
        assert!(
            self.host.received.iter().any(|heard| heard == words),
            "the link is dead"
        );
    }
}

fn untouched(datagram: Vec<u8>) -> Vec<Vec<u8>> {
    vec![datagram]
}

/// Each datagram of the handshake, DCEP included, lost or doubled alone, one
/// way then the other: the channel opens within the window and nothing
/// panics. `handshake.rs` only ever saw a perfect network; a page's `onopen`
/// must not hang on any one datagram, `DATA_CHANNEL_ACK` included, though the
/// channel itself never retransmits (RFC 8832, section 6; `sctp-proto`,
/// `association/stream.rs`).
#[test]
fn no_single_lost_or_doubled_datagram_stops_the_handshake() {
    let mut clean = Link::new();
    clean
        .open(None, &mut untouched)
        .expect("a perfect network opens");
    let (up, down) = (clean.sent_up, clean.sent_down);
    assert!(
        up >= 4 && down >= 4,
        "not a handshake: {up} up, {down} down"
    );
    // `open` can say no: a network that loses everything does not open.
    assert_eq!(Link::new().open(None, &mut |_| Vec::new()), None);
    for (way, sent) in [(Way::Up, up), (Way::Down, down)] {
        for nth in 1..=sent {
            for fault in [Fault::Lost, Fault::Doubled] {
                let plan = Plan { way, nth, fault };
                let opened = Link::new().open(Some(plan), &mut untouched);
                assert!(opened.is_some(), "{plan:?} of {sent} kept it from opening");
            }
        }
    }
}

/// Splits a one-record handshake datagram into two records, each with a
/// fragment of its message: what Chromium 151 does to its ClientHello.
fn in_two_fragments(datagram: Vec<u8>) -> Vec<Vec<u8>> {
    const RECORD: usize = 13;
    const MESSAGE: usize = 12;
    let handshake_record = datagram.first() == Some(&22) && datagram.len() > RECORD + MESSAGE + 2;
    let length = u16::from_be_bytes([datagram[11], datagram[12]]);
    if !handshake_record || usize::from(length) + RECORD != datagram.len() {
        return vec![datagram];
    }
    let body = &datagram[RECORD + MESSAGE..];
    let half = body.len() / 2;
    let fragment = |offset: usize, bytes: &[u8], sequence_bump: u8| {
        let mut record = datagram[..RECORD + MESSAGE].to_vec();
        record[10] = record[10].wrapping_add(sequence_bump);
        let size = u16::try_from(MESSAGE + bytes.len()).expect("a small record");
        record[11..13].copy_from_slice(&size.to_be_bytes());
        // The fragment's offset and length, on three bytes each.
        let offset = u32::try_from(offset).expect("a small message");
        let length = u32::try_from(bytes.len()).expect("a small message");
        record[RECORD + 6..RECORD + 9].copy_from_slice(&offset.to_be_bytes()[1..]);
        record[RECORD + 9..RECORD + 12].copy_from_slice(&length.to_be_bytes()[1..]);
        record.extend_from_slice(bytes);
        record
    };
    vec![
        fragment(0, &body[..half], 0),
        fragment(half, &body[half..], 128),
    ]
}

/// Chromium 151 sends its ClientHello over two datagrams; dimpl must put it
/// back together. Nothing fixed that outside a browser.
#[test]
fn a_client_hello_in_two_fragments_still_opens() {
    let mut split = 0;
    let opened = Link::new().open(None, &mut |datagram| {
        let is_client_hello = datagram.first() == Some(&22) && datagram.get(13) == Some(&1);
        if is_client_hello {
            let fragments = in_two_fragments(datagram);
            split += fragments.len() - 1;
            fragments
        } else {
            vec![datagram]
        }
    });
    assert!(opened.is_some(), "a fragmented ClientHello did not open");
    assert!(split > 0, "nothing was fragmented: this tested nothing");
}

/// `tjunk`'s five forgeries, from the page's own address once the channel is
/// open: an alert in the clear, the ClientHello again, a wrong MAC, noise, a
/// truncated record. None of them may cost a live link: all DTLS errors are
/// fatal to the instance, and the host must not hand it any.
#[test]
fn forged_records_do_not_close_a_live_link() {
    let wrong_mac = || {
        let mut record = vec![23, 0xfe, 0xfd, 0, 1, 0, 0, 0, 0, 1, 0, 0, 64];
        record.extend((0..64_u8).map(|byte| byte.wrapping_mul(37)));
        record
    };
    let noise: Vec<u8> = std::iter::once(25)
        .chain((0..80_u8).map(|byte| byte.wrapping_mul(53)))
        .collect();
    for (name, forged) in [
        (
            "alert",
            vec![21, 0xfe, 0xfd, 0, 0, 0, 0, 0, 0, 0, 200, 0, 2, 2, 40],
        ),
        ("wrong MAC", wrong_mac()),
        ("noise", noise),
        (
            "truncated",
            vec![22, 0xfe, 0xfd, 0, 0, 0, 0, 0, 0, 0, 9, 0x40, 0],
        ),
    ] {
        let mut link = Link::new();
        link.open(None, &mut untouched).expect("a clean opening");
        link.deliver(page_address(), host_address(), &forged);
        assert!(link.host.rtc.is_alive(), "{name} killed the host's end");
        link.speaks(name.as_bytes());
    }
    let mut hello = None;
    let mut link = Link::new();
    link.open(None, &mut |datagram| {
        if hello.is_none() && DTLS.contains(&datagram[0]) {
            hello = Some(datagram.clone());
        }
        vec![datagram]
    })
    .expect("a clean opening");
    let hello = hello.expect("a ClientHello went by");
    link.deliver(page_address(), host_address(), &hello);
    assert!(link.host.rtc.is_alive(), "a replay killed the host's end");
    link.speaks(b"replayed");
}

/// The bytes of the first record of epoch 1 in `datagram`.
fn first_encrypted_record(datagram: &[u8]) -> Option<Vec<u8>> {
    if !DTLS.contains(datagram.first()?) {
        return None;
    }
    let mut at = 0;
    while let Some(header) = datagram.get(at..at + 13) {
        let epoch = u16::from_be_bytes([header[3], header[4]]);
        let end = at + 13 + usize::from(u16::from_be_bytes([header[11], header[12]]));
        let record = datagram.get(at..end)?;
        if epoch == 1 {
            return Some(record.to_vec());
        }
        at = end;
    }
    None
}

/// dimpl #189, what T1 of `recherche/transport-webrtc.toml` hangs on: the page's
/// first encrypted record, alone and twice, with nothing that comes before
/// it. Nobody needs a token for this. The copy of dimpl that str0m 0.24.1
/// brings keeps the second one in a queue and then asserts on it.
///
/// This is no wish: it pins the panic that `salon-host`'s `Entry::guarded` is
/// there for (`rtc_faults.rs`), and what makes it cost one link. It goes red
/// the day str0m brings dimpl 0.9 or later: that day, `PANICS` becomes
/// `false`, and the guard stays, since a dependency can always panic.
#[test]
fn dimpl_on_a_doubled_early_encrypted_record() {
    /// dimpl 0.7.6, `src/dtls12/engine.rs:381`.
    const PANICS: bool = true;

    let mut link = Link::new();
    let mut wire = VecDeque::new();
    let mut held = None;
    while held.is_none() && link.now - link.start <= WINDOW {
        link.page.drain(Way::Up, &mut wire);
        link.host.drain(Way::Down, &mut wire);
        let Some((way, source, destination, contents)) = wire.pop_front() else {
            link.tick();
            continue;
        };
        match (way, first_encrypted_record(&contents)) {
            (Way::Up, Some(record)) => held = Some((source, destination, record)),
            (Way::Up | Way::Down, _) => link.deliver(source, destination, &contents),
        }
    }
    let (source, destination, record) = held.expect("the page reached its Finished");
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        link.deliver(source, destination, &record);
        link.deliver(source, destination, &record);
        link.host.rtc.poll_output().expect("str0m output");
    }))
    .is_err();
    assert_eq!(panicked, PANICS);
}
