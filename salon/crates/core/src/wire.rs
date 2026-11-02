//! The packets on the wire, in both directions. One packet per send carries
//! everything due: acknowledgements, fragments of events and of the state.
//! Encoded with postcard; anything that does not decode is ignored.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::stream::Order;
use crate::types::{DeviceId, Kind, Refusal, Token, TvKey};

/// Bumped whenever a packet changes shape. Both sides refuse another one: a
/// host reads the number even in a hello it can no longer read
/// ([`another_protocol`]), and `Refused` keeps its shape for that reason.
pub const PROTOCOL: u16 = 2;

/// Bytes a packet spends outside its fragments (generous: fields are
/// varints), and bytes each fragment spends besides its data.
pub(crate) const PACKET_OVERHEAD: usize = 112;
pub(crate) const PART_OVERHEAD: usize = 40;

/// Largest packet on any link. A bigger one closes its link.
pub(crate) const MAX_PACKET: usize = 64 * 1024;
/// Parts a packet can hold: what a sender spends on each one bounds them, and
/// a packet with more was not cut by salon.
const MAX_PARTS: usize = (MAX_PACKET - PACKET_OVERHEAD) / PART_OVERHEAD;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Packet {
    /// The host's epoch as the sender knows it (0 before the first welcome).
    pub epoch: u32,
    /// The sender's clock when it sent this.
    pub sent: u64,
    /// The last `sent` the sender heard, and how long it held it since.
    pub echo: Option<Echo>,
    pub control: Option<Control>,
    pub ack: Option<Ack>,
    /// Host to device: the game's current phase. Events carry back the phase
    /// their sender had seen.
    pub phase: Option<u32>,
    /// Device to host: the state the device still holds, so the host knows
    /// a held input is still meant without receiving it again.
    pub holding: Option<u32>,
    pub events: Vec<EventPart>,
    pub state: Vec<StatePart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Echo {
    pub sent: u64,
    pub held: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Ack {
    /// Every event below this number arrived.
    pub events: u32,
    /// The latest state applied (0: none yet).
    pub state: u32,
    /// Bit `n` says event `events + n` is here whole, waiting for its turn.
    pub ahead: u64,
    /// What arrived of the first event not whole.
    pub event_parts: Option<Parts>,
    /// What arrived of the state being put back together.
    pub state_parts: Option<Parts>,
}

impl Ack {
    /// It says more than what is below its numbers.
    fn says_more(&self) -> bool {
        self.ahead != 0 || self.event_parts.is_some() || self.state_parts.is_some()
    }
}

/// The fragments that arrived of one message not whole yet, so that a resend
/// carries the others. A window that starts at the first missing fragment
/// keeps this small and bounded: what lies past it goes again, and is ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Parts {
    pub seq: u32,
    /// Every fragment below this one arrived, and this one did not.
    pub first: u16,
    /// Bit `n` says fragment `first + 1 + n` arrived.
    pub more: u64,
}

impl Parts {
    /// Fragments this says enough of to tell which are not there: up to the
    /// last one that arrived, or on a stream, where nothing can still be on
    /// its way, all of those it has a bit for.
    pub fn span(self, order: Order) -> usize {
        let past = match order {
            Order::Any => u64::BITS.saturating_sub(self.more.leading_zeros()),
            Order::Kept => u64::BITS.saturating_add(1),
        };
        usize::from(self.first).saturating_add(usize::try_from(past).unwrap_or(0))
    }
}

/// One fragment of one event. Events are numbered per device and per epoch
/// from 0; a message too big for one packet is cut in `count` fragments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EventPart {
    pub seq: u32,
    pub index: u16,
    pub count: u16,
    /// The phase the sender had seen when it sent the event.
    pub phase: u32,
    /// The sender's estimate of the host's clock when it sent the event.
    pub at: Option<u64>,
    pub data: Vec<u8>,
}

/// One fragment of the latest state. States are numbered from 1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StatePart {
    pub seq: u32,
    pub index: u16,
    pub count: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Control {
    /// Device to host, first on every link and repeated until welcomed.
    Hello(Hello),
    /// Host to device, in answer to each `Hello`.
    Welcome(Welcome),
    /// Host to device: turned away. Final.
    Refused(Refusal),
    /// Host to device: another link took this device over. Final: the phone
    /// does not reconnect on its own, or two tabs would evict each other in
    /// a loop.
    Replaced,
    /// Device to host: the player leaves.
    Bye,
    /// Host to device: the host is stopping. Keep trying to come back.
    Closing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hello {
    pub protocol: u16,
    /// The game's own version: the host refuses another one.
    pub game: u32,
    pub kind: Kind,
    pub token: Token,
    pub name: Option<String>,
    pub key: Option<TvKey>,
    /// Where this client last was, if anywhere: when it is where the device
    /// still is, the host and the client keep their numbering.
    pub resume: Option<Resume>,
}

/// Where a client stands: which run of the host, as which device, and which
/// of the clients that held this device. Two tabs, or a page and its reload,
/// show one token: only the host can tell them apart, and the numbers of one
/// mean nothing to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Resume {
    pub epoch: u32,
    pub device: DeviceId,
    pub session: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Welcome {
    pub device: DeviceId,
    pub epoch: u32,
    /// Changes when another client takes the device over: a client with
    /// another number renumbers what waits from `up_next`.
    pub session: u32,
    /// The next event number the host expects from this device.
    pub up_next: u32,
    /// What arrived of the session the hello named: its events below this
    /// number. 0 when the host cannot tell: they go again.
    pub up_seen: u32,
    /// The latest state number the host took from this device: a device that
    /// starts over numbers above it.
    pub state: u32,
    /// The oldest event the host still has for this device.
    pub down_first: u32,
    /// The largest message the host takes, and sends: its limits are the room's,
    /// and a page does not choose them.
    pub max_up: u32,
    pub max_down: u32,
}

/// What every protocol's hello starts with, whatever comes after: up to the
/// tag of [`Control::Hello`] and its first field.
#[derive(Deserialize)]
enum HelloHead {
    Hello(u16),
}

/// These bytes do not read as a packet, but they start like a hello of
/// another protocol: a page left open through an upgrade. Without this, a
/// page that resumes would be garbage and reconnect for ever.
pub(crate) fn another_protocol(bytes: &[u8]) -> bool {
    type Head = (u32, u64, Option<Echo>, Option<HelloHead>);
    postcard::take_from_bytes::<Head>(bytes).is_ok_and(|((_, _, _, hello), _)| {
        matches!(hello, Some(HelloHead::Hello(protocol)) if protocol != PROTOCOL)
    })
}

pub(crate) fn encode(packet: &Packet) -> Option<Vec<u8>> {
    postcard::to_allocvec(packet).ok()
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Packet> {
    let packet: Packet = crate::codec::decode(bytes).ok()?;
    let parts = packet.events.len().saturating_add(packet.state.len());
    (parts <= MAX_PARTS).then_some(packet)
}

/// Encodes `packet`, dropping its last fragments until it fits `max`. The
/// builders size fragments so this never happens; if it did, a dropped
/// fragment counts as lost and goes again at the next resend. `None` would
/// lose its control message in silence: every one of them, at its largest,
/// fits [`Caps::MIN_PACKET`](crate::types::Caps::MIN_PACKET), and
/// `every_control_message_fits_the_smallest_packet` holds that true.
pub(crate) fn encode_within(mut packet: Packet, max: usize) -> Option<Vec<u8>> {
    loop {
        let bytes = encode(&packet)?;
        if bytes.len() <= max {
            return Some(bytes);
        }
        if packet.state.pop().is_none() && packet.events.pop().is_none() {
            // What arrived of a message saves bytes and nothing depends on
            // it: without it, the sender sends those fragments again.
            let Some(ack) = packet.ack.as_mut().filter(|ack| ack.says_more()) else {
                // Headers alone do not fit: nothing sensible to send.
                return None;
            };
            ack.ahead = 0;
            ack.event_parts = None;
            ack.state_parts = None;
        }
    }
}

#[cfg(test)]
mod framing_tests {
    use alloc::vec;

    use super::*;
    use crate::types::{Caps, Name, Refusal};

    #[test]
    fn packet_boundary_does_not_accept_a_valid_prefix() {
        let packet = Packet::default();
        let mut bytes = encode(&packet).unwrap();
        assert_eq!(decode(&bytes), Some(packet));
        bytes.push(0);
        assert!(decode(&bytes).is_none());
    }

    const LARGEST_PARTS: Parts = Parts {
        seq: u32::MAX,
        first: u16::MAX,
        more: u64::MAX,
    };

    /// Past the bits of an ack nothing is known, even on a stream: those
    /// fragments stay on their way until a later ack names them.
    #[test]
    fn an_ack_tells_of_the_fragments_it_has_a_bit_for_and_no_more() {
        let parts = Parts {
            seq: 0,
            first: 3,
            more: 0b100,
        };
        assert_eq!(parts.span(Order::Any), 6);
        assert_eq!(parts.span(Order::Kept), 68);
    }

    /// A fragment cut for this overhead must fit beside the largest ack: a
    /// packet that could not hold both would leave without its fragment, every
    /// time.
    #[test]
    fn the_largest_headers_fit_the_overhead() {
        let headers = Packet {
            control: None,
            events: Vec::new(),
            state: Vec::new(),
            ..largest(Control::Bye)
        };
        assert!(encode(&headers).expect("headers encode").len() <= PACKET_OVERHEAD);
    }

    /// A packet with every optional field at its largest number, around
    /// `control`, and more data than any packet takes.
    fn largest(control: Control) -> Packet {
        let data = vec![0_u8; Caps::MIN_PACKET];
        Packet {
            epoch: u32::MAX,
            sent: u64::MAX,
            echo: Some(Echo {
                sent: u64::MAX,
                held: u32::MAX,
            }),
            control: Some(control),
            ack: Some(Ack {
                events: u32::MAX,
                state: u32::MAX,
                ahead: u64::MAX,
                event_parts: Some(LARGEST_PARTS),
                state_parts: Some(LARGEST_PARTS),
            }),
            phase: Some(u32::MAX),
            holding: Some(u32::MAX),
            events: vec![EventPart {
                seq: u32::MAX,
                index: u16::MAX,
                count: u16::MAX,
                phase: u32::MAX,
                at: Some(u64::MAX),
                data: data.clone(),
            }],
            state: vec![StatePart {
                seq: u32::MAX,
                index: u16::MAX,
                count: u16::MAX,
                data,
            }],
        }
    }

    /// `encode_within` gives up only on what does not fit without its
    /// fragments, and that would drop a hello or a refusal in silence: a phone
    /// would wait for ever. The largest of each must fit the smallest packet.
    #[test]
    fn every_control_message_fits_the_smallest_packet() {
        let widest = "\u{1f600}".repeat(Name::MAX_CHARS * 2);
        let name = Name::new(&widest).expect("a name");
        let max = Resume {
            epoch: u32::MAX,
            device: DeviceId(u32::MAX),
            session: u32::MAX,
        };
        let controls = [
            Control::Hello(Hello {
                protocol: u16::MAX,
                game: u32::MAX,
                kind: Kind::Keyboard,
                token: Token([u8::MAX; 16]),
                name: Some(String::from(name.as_str())),
                key: Some(TvKey([u8::MAX; 16])),
                resume: Some(max),
            }),
            Control::Welcome(Welcome {
                device: DeviceId(u32::MAX),
                epoch: u32::MAX,
                session: u32::MAX,
                up_next: u32::MAX,
                up_seen: u32::MAX,
                state: u32::MAX,
                down_first: u32::MAX,
                max_up: u32::MAX,
                max_down: u32::MAX,
            }),
            Control::Refused(Refusal::Removed),
            Control::Replaced,
            Control::Bye,
            Control::Closing,
        ];
        for control in controls {
            let with_state = largest(control.clone());
            let without_state = Packet {
                state: Vec::new(),
                ..with_state.clone()
            };
            for packet in [with_state, without_state] {
                let bytes =
                    encode_within(packet, Caps::MIN_PACKET).expect("a control message fits");
                assert_eq!(
                    decode(&bytes).and_then(|packet| packet.control),
                    Some(control.clone())
                );
            }
        }
    }
}
