//! WebRTC links: every page's DataChannel, on one UDP port, driven by str0m
//! in ICE-lite. The host announces its network address and its loopback one
//! (a page of this very computer reaches it without the firewall); it learns
//! each phone's from its first packet (Safari hides its own behind a `.local`
//! name). The cryptography is salon-crypto's, in Rust only.
//!
//! str0m wants every input followed by draining its outputs before the next
//! input (`str0m/src/lib.rs`, "single-mutation invariant"): [`drive`] does
//! that after each one.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use salon_core::{Caps, LinkId};
use salon_crypto::DtlsCert;
use str0m::change::SdpOffer;
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc, RtcConfig};

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

pub struct Signal {
    pub link: LinkId,
    pub rtc: Rtc,
}

/// The loopback address on the same port.
fn loopback(address: SocketAddr) -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), address.port())
}

/// Answers a page's offer (JSON, as `RTCSessionDescription`) with a new
/// connection in ICE-lite, with the certificate made at start.
pub fn answer(
    offer: &[u8],
    address: SocketAddr,
    cert: &DtlsCert,
) -> Result<(Rtc, Vec<u8>), String> {
    let offer: SdpOffer =
        serde_json::from_slice(offer).map_err(|error| format!("bad offer: {error}"))?;
    let mut rtc = RtcConfig::new()
        .set_ice_lite(true)
        .set_dtls_cert(cert.clone())
        .build(Clock::instant());
    // The network address for phones, and the loopback one for pages of this
    // very computer: a phone ignores a candidate it cannot reach.
    for candidate in [address, loopback(address)] {
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
    /// The network address announced to pages (with its loopback twin).
    pub address: SocketAddr,
    pub signals: Receiver<Signal>,
    pub outgoing: Receiver<RtcOut>,
    pub outlet: SyncSender<RtcOut>,
    pub net: ToRoom,
}

pub fn run(rtcs: &Rtcs) {
    if rtcs.socket.set_read_timeout(Some(POLL)).is_err() {
        crate::say(format_args!(
            "salon: cannot set the UDP socket's timeout; WebRTC is off"
        ));
        return;
    }
    let mut entries: BTreeMap<LinkId, Entry> = BTreeMap::new();
    let mut buffer = vec![0_u8; 2048];
    loop {
        rtcs.net.flush();
        loop {
            match rtcs.signals.try_recv() {
                Ok(Signal { link, rtc }) => {
                    let mut entry = Entry {
                        rtc,
                        channel: None,
                        created: Clock::instant(),
                        timeout: None,
                        local: false,
                    };
                    drive(rtcs, link, &mut entry);
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
                        if let Some(mut channel) = entry.rtc.channel(id) {
                            // `false`: str0m's buffer is full; the packet is
                            // lost and salon sends it again.
                            let _: Result<bool, str0m::RtcError> = channel.write(true, &bytes);
                        }
                        drive(rtcs, link, entry);
                    }
                }
                RtcOut::Close(link) => {
                    if let Some(mut entry) = entries.remove(&link) {
                        entry.rtc.disconnect();
                    }
                }
            }
        }
        receive(rtcs, &mut entries, &mut buffer);
        let now = Clock::instant();
        for (&link, entry) in &mut entries {
            if entry.timeout.is_some_and(|at| at <= now) {
                if entry.rtc.handle_input(Input::Timeout(now)).is_err() {
                    entry.rtc.disconnect();
                }
                drive(rtcs, link, entry);
            }
        }
        entries.retain(|&link, entry| {
            let stalled = entry.channel.is_none() && now.duration_since(entry.created) > HANDSHAKE;
            let keep = entry.rtc.is_alive() && !stalled;
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
fn receive(rtcs: &Rtcs, entries: &mut BTreeMap<LinkId, Entry>, buffer: &mut [u8]) {
    let (size, source) = match rtcs.socket.recv_from(buffer) {
        Ok(read) => read,
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return,
        Err(error) => {
            crate::say(format_args!("salon: UDP: {error}"));
            return;
        }
    };
    let Some(Ok(contents)) = buffer.get(..size).map(<&[u8]>::try_into) else {
        return;
    };
    // The socket listens on every interface: a packet from this computer
    // came to the loopback candidate.
    let local = source.ip().is_loopback();
    let input = Input::Receive(
        Clock::instant(),
        Receive {
            proto: Protocol::Udp,
            source,
            destination: if local {
                loopback(rtcs.address)
            } else {
                rtcs.address
            },
            contents,
        },
    );
    let Some((&link, entry)) = entries
        .iter_mut()
        .find(|(_, entry)| entry.rtc.accepts(&input))
    else {
        return;
    };
    entry.local = local;
    if entry.rtc.handle_input(input).is_ok() {
        drive(rtcs, link, entry);
    } else {
        entry.rtc.disconnect();
    }
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
mod tests {
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;

    use super::*;
    use crate::room::inbox::channel;

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
