//! A second, independent reading of the bytes on the wire, for tests that must
//! see or forge a packet. Salon's own types stay private; this copy is field
//! for field, so a change of shape without a new `PROTOCOL` fails these tests.

#![expect(
    clippy::expect_used,
    reason = "test helpers outside #[test] functions: a packet that does not read stops the test"
)]

use salon_core::{DeviceId, Kind, Refusal, Token, TvKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Packet {
    pub epoch: u32,
    pub sent: u64,
    pub echo: Option<Echo>,
    pub control: Option<Control>,
    pub ack: Option<Ack>,
    pub phase: Option<u32>,
    pub holding: Option<u32>,
    pub events: Vec<EventPart>,
    pub state: Vec<StatePart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Echo {
    pub sent: u64,
    pub held: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    pub events: u32,
    pub state: u32,
    pub ahead: u64,
    pub event_parts: Option<Parts>,
    pub state_parts: Option<Parts>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parts {
    pub seq: u32,
    pub first: u16,
    pub more: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPart {
    pub seq: u32,
    pub index: u16,
    pub count: u16,
    pub phase: u32,
    pub at: Option<u64>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatePart {
    pub seq: u32,
    pub index: u16,
    pub count: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    Hello(Hello),
    Welcome(Welcome),
    Refused(Refusal),
    Replaced,
    Bye,
    Closing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u16,
    pub game: u32,
    pub kind: Kind,
    pub token: Token,
    pub name: Option<String>,
    pub key: Option<TvKey>,
    pub resume: Option<Resume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resume {
    pub epoch: u32,
    pub device: DeviceId,
    pub session: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    pub device: DeviceId,
    pub epoch: u32,
    pub session: u32,
    pub up_next: u32,
    pub up_seen: u32,
    pub state: u32,
    pub down_first: u32,
}

pub fn encode(packet: &Packet) -> Vec<u8> {
    postcard::to_allocvec(packet).expect("a packet encodes")
}

pub fn decode(bytes: &[u8]) -> Packet {
    salon_core::codec::decode(bytes).expect("a packet from salon reads")
}

/// A packet from the host's run `epoch`, with nothing in it.
pub fn from_host(epoch: u32, sent: u64) -> Packet {
    Packet {
        epoch,
        sent,
        ..Packet::default()
    }
}

pub fn welcome(device: u32, epoch: u32, up_next: u32, down_first: u32) -> Vec<u8> {
    encode(&Packet {
        control: Some(Control::Welcome(Welcome {
            device: DeviceId(device),
            epoch,
            session: 0,
            up_next,
            up_seen: 0,
            state: 0,
            down_first,
        })),
        ..from_host(epoch, 0)
    })
}

/// A whole message in one fragment.
pub fn event(seq: u32, data: Vec<u8>) -> EventPart {
    EventPart {
        seq,
        index: 0,
        count: 1,
        phase: 0,
        at: None,
        data,
    }
}

pub fn state(seq: u32, data: Vec<u8>) -> StatePart {
    StatePart {
        seq,
        index: 0,
        count: 1,
        data,
    }
}
