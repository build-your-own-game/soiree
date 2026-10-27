//! The packets on the wire, in both directions. One packet per send carries
//! everything due: acknowledgements, fragments of events and of the state.
//! Encoded with postcard; anything that does not decode is ignored.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::types::{DeviceId, Kind, Refusal, Token, TvKey};

/// Bumped whenever a packet changes shape. Both sides refuse another one.
pub const PROTOCOL: u16 = 1;

/// Bytes a packet spends outside its fragments (generous: fields are
/// varints), and bytes each fragment spends besides its data.
pub(crate) const PACKET_OVERHEAD: usize = 64;
pub(crate) const PART_OVERHEAD: usize = 40;

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
    /// The epoch and device this client last knew, if any: with them, the
    /// host and the client keep their numbering and nothing is lost.
    pub resume: Option<(u32, DeviceId)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Welcome {
    pub device: DeviceId,
    pub epoch: u32,
    /// The next event number the host expects from this device.
    pub up_next: u32,
    /// The oldest event the host still has for this device.
    pub down_first: u32,
}

pub(crate) fn encode(packet: &Packet) -> Option<Vec<u8>> {
    postcard::to_allocvec(packet).ok()
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Packet> {
    postcard::from_bytes(bytes).ok()
}

/// Encodes `packet`, dropping its last fragments until it fits `max`. The
/// builders size fragments so this never happens; if it did, a dropped
/// fragment counts as lost and goes again at the next resend.
pub(crate) fn encode_within(mut packet: Packet, max: usize) -> Option<Vec<u8>> {
    loop {
        let bytes = encode(&packet)?;
        if bytes.len() <= max {
            return Some(bytes);
        }
        if packet.state.pop().is_none() && packet.events.pop().is_none() {
            // Headers alone do not fit: nothing sensible to send.
            return None;
        }
    }
}
