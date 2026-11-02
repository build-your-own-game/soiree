//! What both targets share: a room, one honest phone, and the exchange
//! between them. Packets are private to salon-core, so everything goes through
//! the public calls a shell uses, and valid bytes come from a real `Client`.

use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind,
    Limits, LinkId, Ms, Output, Timing, Token, TvKey,
};
use serde::{Deserialize, Serialize};

pub const GAME: u32 = 7;
pub const HONEST: LinkId = LinkId(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Up {
    Tap(u8),
    Pad(i8, i8),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Down {
    View(u8),
    Note(u8),
    Blob(Vec<u8>),
}

pub type Room = Host<Up, Down>;
pub type Phone = Client<Up, Down>;

pub fn room(now: Ms) -> Room {
    Host::new(
        now,
        HostConfig {
            game: GAME,
            epoch: Epoch(5),
            tv_key: TvKey([9; 16]),
            timing: Timing::default(),
            limits: Limits::default(),
            known: Vec::new(),
        },
    )
}

pub fn phone(now: Ms, token: Token) -> Phone {
    Client::new(
        now,
        ClientConfig {
            game: GAME,
            kind: Kind::Phone,
            token,
            name: Some("fuzz".to_owned()),
            tv_key: None,
            seed: u64::from(token.0[0]),
            timing: Timing::default(),
            limits: Limits::default(),
        },
    )
}

/// Everything the room did, sorted out; every transmission obeys its link.
#[derive(Default)]
pub struct Heard {
    pub events: Vec<DeviceEvent<Up>>,
    pub to_links: Vec<(LinkId, Vec<u8>)>,
    pub closed: Vec<LinkId>,
}

pub fn drain_room(room: &mut Room, now: Ms) -> Heard {
    let mut heard = Heard::default();
    while let Some(output) = room.poll(now) {
        match output {
            Output::Transmit { link, bytes } => {
                assert!(
                    bytes.len() <= Caps::DATAGRAM.max_packet,
                    "{} bytes for a link of {}",
                    bytes.len(),
                    Caps::DATAGRAM.max_packet
                );
                heard.to_links.push((link, bytes));
            }
            Output::Close { link } => heard.closed.push(link),
            Output::Device(event) => {
                if let DeviceEvent::Event { at, .. } = &event {
                    assert!(*at <= now, "an event dated {at:?} at {now:?}");
                }
                heard.events.push(event);
            }
        }
    }
    heard
}

/// One honest phone connected to the room over `HONEST`, handshake done.
pub fn joined(now: Ms, token: Token) -> (Room, Phone, DeviceId) {
    let mut host = room(now);
    let mut client = phone(now, token);
    for _ in 0..4 {
        while let Some(output) = client.poll(now) {
            match output {
                ClientOutput::Connect => {
                    host.opened(now, HONEST, Caps::DATAGRAM);
                    client.opened(now, Caps::DATAGRAM);
                }
                ClientOutput::Transmit(bytes) => host.received(now, HONEST, &bytes),
                ClientOutput::Disconnect
                | ClientOutput::Status(_)
                | ClientOutput::Reset
                | ClientOutput::Event(_)
                | ClientOutput::State(_) => {}
            }
        }
        for (link, bytes) in drain_room(&mut host, now).to_links {
            if link == HONEST {
                client.received(now, &bytes);
            }
        }
    }
    let device = client.device().expect("the honest phone joins");
    (host, client, device)
}
