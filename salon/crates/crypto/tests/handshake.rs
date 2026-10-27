//! The provider does real WebRTC: a client like a browser page and a host in
//! ICE-lite like salon-host, both str0m, connect over an in-memory network
//! in virtual time, go through ICE, DTLS (with salon's own certificates) and
//! SCTP, then exchange messages on an unordered DataChannel without
//! retransmission, as salon does.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers outside #[test] functions: a failed step stops the test with its message"
)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc, RtcConfig};

const PAGE: &str = "192.168.1.20:50000";
const HOST: &str = "192.168.1.10:8090";

struct Side {
    rtc: Rtc,
    address: SocketAddr,
    timeout: Instant,
    opened: Option<ChannelId>,
    received: Vec<Vec<u8>>,
}

/// Drains `side` until it asks for a timeout; its packets go to `wire`.
fn drain(side: &mut Side, wire: &mut VecDeque<(SocketAddr, SocketAddr, Vec<u8>)>) {
    loop {
        match side.rtc.poll_output().expect("str0m output") {
            Output::Timeout(at) => {
                side.timeout = at;
                return;
            }
            Output::Transmit(transmit) => wire.push_back((
                transmit.source,
                transmit.destination,
                transmit.contents.to_vec(),
            )),
            Output::Event(Event::ChannelOpen(id, _)) => side.opened = Some(id),
            Output::Event(Event::ChannelData(data)) => side.received.push(data.data),
            Output::Event(_) => {}
        }
    }
}

/// Runs the two sides until `done`, at most 30 virtual seconds.
fn run(
    page: &mut Side,
    host: &mut Side,
    start: Instant,
    mut done: impl FnMut(&mut Side, &mut Side) -> bool,
) {
    let mut now = start;
    let mut wire = VecDeque::new();
    while now < start + Duration::from_secs(30) {
        drain(page, &mut wire);
        drain(host, &mut wire);
        while let Some((source, destination, contents)) = wire.pop_front() {
            let side = if destination == host.address {
                &mut *host
            } else {
                &mut *page
            };
            let input = Input::Receive(
                now,
                Receive {
                    proto: Protocol::Udp,
                    source,
                    destination,
                    contents: contents.as_slice().try_into().expect("a datagram"),
                },
            );
            side.rtc.handle_input(input).expect("str0m input");
            drain(side, &mut wire);
        }
        if done(page, host) {
            return;
        }
        now = page
            .timeout
            .min(host.timeout)
            .max(now + Duration::from_millis(1));
        for side in [&mut *page, &mut *host] {
            side.rtc
                .handle_input(Input::Timeout(now))
                .expect("str0m timeout");
        }
    }
    panic!("did not finish within 30 virtual seconds");
}

#[test]
fn a_page_and_the_host_talk_over_dtls_with_salons_certificates() {
    salon_crypto::provider().install_process_default();
    #[expect(
        clippy::disallowed_methods,
        reason = "str0m wants a real Instant to start from; time is virtual after"
    )]
    let start = Instant::now();

    let page_address: SocketAddr = PAGE.parse().expect("an address");
    let host_address: SocketAddr = HOST.parse().expect("an address");
    let mut page = Side {
        rtc: RtcConfig::new().build(start),
        address: page_address,
        timeout: start,
        opened: None,
        received: Vec::new(),
    };
    let mut host = Side {
        rtc: RtcConfig::new().set_ice_lite(true).build(start),
        address: host_address,
        timeout: start,
        opened: None,
        received: Vec::new(),
    };
    page.rtc
        .add_local_candidate(Candidate::host(page_address, "udp").expect("a candidate"));
    host.rtc
        .add_local_candidate(Candidate::host(host_address, "udp").expect("a candidate"));

    // What salon-web asks the browser for.
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

    run(&mut page, &mut host, start, |page, host| {
        page.opened.is_some() && host.opened.is_some()
    });

    let (Some(page_channel), Some(host_channel)) = (page.opened, host.opened) else {
        panic!("the channel did not open on both sides");
    };
    page.rtc
        .channel(page_channel)
        .expect("the page's channel")
        .write(true, b"salut")
        .expect("the page writes");
    host.rtc
        .channel(host_channel)
        .expect("the host's channel")
        .write(true, b"bienvenue")
        .expect("the host writes");
    run(&mut page, &mut host, start, |page, host| {
        !page.received.is_empty() && !host.received.is_empty()
    });
    assert_eq!(host.received, vec![b"salut".to_vec()]);
    assert_eq!(page.received, vec![b"bienvenue".to_vec()]);
}

#[test]
fn every_certificate_is_new_and_well_formed() {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;

    let first = salon_crypto::self_signed().expect("a certificate");
    let second = salon_crypto::self_signed().expect("another one");
    let first_cert = Certificate::from_der(&first.certificate).expect("valid DER");
    let second_cert = Certificate::from_der(&second.certificate).expect("valid DER");
    // Firefox refuses two certificates with the same serial (str0m #517).
    assert_ne!(
        first_cert.tbs_certificate.serial_number,
        second_cert.tbs_certificate.serial_number
    );
    assert_ne!(first.private_key, second.private_key);
}

/// BoringSSL (Chrome, Safari's WebRTC) refuses to use a certificate for the
/// DTLS signature unless its key usage allows signing; str0m does not check,
/// so only a browser would notice. An end entity that may sign.
#[test]
fn the_certificate_may_sign_and_is_no_authority() {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    use x509_cert::ext::pkix::{BasicConstraints, KeyUsage, KeyUsages};

    let cert = salon_crypto::self_signed().expect("a certificate");
    let cert = Certificate::from_der(&cert.certificate).expect("valid DER");
    let usage: KeyUsage = cert
        .tbs_certificate
        .get()
        .expect("readable extensions")
        .map(|(_, usage)| usage)
        .expect("a key usage");
    assert!(usage.0.contains(KeyUsages::DigitalSignature), "{usage:?}");
    assert!(!usage.0.contains(KeyUsages::KeyCertSign), "{usage:?}");
    let constraints: Option<(bool, BasicConstraints)> =
        cert.tbs_certificate.get().expect("readable extensions");
    assert!(constraints.is_none_or(|(_, constraints)| !constraints.ca));
}
