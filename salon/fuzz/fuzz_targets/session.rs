//! A hostile device and an honest phone share one room. The hostile one sends
//! raw bytes, tampered copies of its real packets and replays; the game and
//! the honest phone go on with their evening. The room never panics, and the
//! honest phone stays connected and gets, and gives, exactly what it should:
//! once, in order, whatever the other link did (E1, E3 of CERTIFICATION.md).
#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use salon_core::journal::fingerprint;
use salon_core::{Caps, ClientOutput, DeviceEvent, LinkId, Ms, Status, Token};

mod common;

use common::{Down, HONEST, Phone, Room, Up, drain_room, joined, phone};

const HOSTILE: LinkId = LinkId(2);
const STRANGER: LinkId = LinkId(3);
const STEPS: usize = 256;

#[derive(Arbitrary, Debug)]
enum Op {
    /// Raw bytes on the hostile link, then on a link that never said hello.
    Hostile(Vec<u8>),
    Stranger(Vec<u8>),
    /// The hostile phone's latest real packet, one byte changed: past the
    /// decoder, into the fields.
    Tamper {
        offset: u16,
        xor: u8,
    },
    Replay,
    /// Valid traffic from the hostile phone, for `Tamper` to work on.
    HostileEvent(Vec<u8>),
    HostileState(Vec<u8>),
    /// The game talks to the hostile device, so its queues move too.
    Show(Vec<u8>),
    HostileClosed,
    Forget,
    /// The honest side of the evening.
    PhoneEvent(u8),
    PhoneState(u8),
    GameEvent(u8),
    GameState(u8),
    Phase(u32),
    Open(bool),
    /// At most 1 020 ms, so the honest phone never goes silent between steps.
    Wait(u8),
}

struct Evening {
    now: Ms,
    room: Room,
    honest: Phone,
    hostile: Phone,
    hostile_open: bool,
    last_real: Option<Vec<u8>>,
    from_honest: Vec<Up>,
    to_honest: Vec<Down>,
    states_to_honest: Vec<Down>,
}

impl Evening {
    fn pump(&mut self) {
        let device = self.honest.device();
        for _ in 0..8 {
            while let Some(output) = self.honest.poll(self.now) {
                match output {
                    ClientOutput::Transmit(bytes) => self.room.received(self.now, HONEST, &bytes),
                    ClientOutput::Event(event) => self.to_honest.push(event),
                    ClientOutput::State(state) => self.states_to_honest.push(state),
                    ClientOutput::Connect | ClientOutput::Disconnect => {
                        panic!("the honest phone lost its link: {output:?}")
                    }
                    ClientOutput::Status(_) | ClientOutput::Reset => {}
                }
            }
            while let Some(output) = self.hostile.poll(self.now) {
                match output {
                    ClientOutput::Connect => {
                        self.room.opened(self.now, HOSTILE, Caps::DATAGRAM);
                        self.hostile.opened(self.now, Caps::DATAGRAM);
                        self.hostile_open = true;
                    }
                    ClientOutput::Transmit(bytes) => {
                        self.room.received(self.now, HOSTILE, &bytes);
                        self.last_real = Some(bytes);
                    }
                    ClientOutput::Disconnect => {
                        self.room.closed(self.now, HOSTILE);
                        self.hostile_open = false;
                    }
                    ClientOutput::Status(_)
                    | ClientOutput::Reset
                    | ClientOutput::Event(_)
                    | ClientOutput::State(_) => {}
                }
            }
            let heard = drain_room(&mut self.room, self.now);
            for event in heard.events {
                match event {
                    DeviceEvent::Event {
                        device: from,
                        event,
                        ..
                    } if Some(from) == device => {
                        self.from_honest.push(event);
                    }
                    DeviceEvent::Left { device: from, why } if Some(from) == device => {
                        panic!("the honest phone left the room: {why:?}")
                    }
                    DeviceEvent::Arrived { .. }
                    | DeviceEvent::Back { .. }
                    | DeviceEvent::Left { .. }
                    | DeviceEvent::Event { .. }
                    | DeviceEvent::State { .. } => {}
                }
            }
            for link in heard.closed {
                if link == HOSTILE {
                    self.hostile.closed(self.now);
                    self.hostile_open = false;
                }
            }
            if heard.to_links.is_empty() {
                return;
            }
            for (link, bytes) in heard.to_links {
                if link == HONEST {
                    self.honest.received(self.now, &bytes);
                } else if link == HOSTILE {
                    self.hostile.received(self.now, &bytes);
                }
            }
        }
    }
}

/// The honest token depends on the whole input: a comparison seen by the
/// fuzzer must not hand it the secret, which nobody has in the real room.
fn secret(data: &[u8]) -> Option<Token> {
    let digest = fingerprint(&data).ok()?;
    digest.as_bytes().first_chunk().copied().map(Token)
}

fuzz_target!(|data: &[u8]| {
    let Some(token) = secret(data) else {
        return;
    };
    let Ok(ops) = Vec::<Op>::arbitrary(&mut Unstructured::new(data)) else {
        return;
    };
    let now = Ms(1_000);
    let (room, honest, device) = joined(now, token);
    let mut evening = Evening {
        now,
        room,
        honest,
        hostile: phone(now, Token([2; 16])),
        hostile_open: false,
        last_real: None,
        from_honest: Vec::new(),
        to_honest: Vec::new(),
        states_to_honest: Vec::new(),
    };
    evening.room.opened(now, STRANGER, Caps::DATAGRAM);
    let (mut sent_up, mut sent_down, mut shown) = (Vec::new(), Vec::new(), None);
    let mut number = 0_u8;
    for op in ops.into_iter().take(STEPS) {
        number = number.wrapping_add(1);
        let now = evening.now;
        let hostile_device = evening.hostile.device();
        match op {
            Op::Hostile(bytes) if evening.hostile_open => {
                evening.room.received(now, HOSTILE, &bytes);
            }
            Op::Stranger(bytes) => evening.room.received(now, STRANGER, &bytes),
            Op::Tamper { offset, xor } if evening.hostile_open => {
                if let Some(mut bytes) = evening.last_real.clone() {
                    if !bytes.is_empty() {
                        let at = usize::from(offset) % bytes.len();
                        if let Some(byte) = bytes.get_mut(at) {
                            *byte ^= xor;
                        }
                    }
                    evening.room.received(now, HOSTILE, &bytes);
                }
            }
            Op::Replay if evening.hostile_open => {
                if let Some(bytes) = &evening.last_real {
                    evening.room.received(now, HOSTILE, bytes);
                }
            }
            Op::HostileEvent(blob) => {
                let _ = evening.hostile.send(&Up::Blob(blob));
            }
            Op::HostileState(blob) => {
                let _ = evening.hostile.set_state(&Up::Blob(blob));
            }
            Op::Show(blob) => {
                if let Some(hostile) = hostile_device {
                    let _ = evening.room.show(hostile, &Down::Blob(blob));
                }
            }
            Op::HostileClosed if evening.hostile_open => {
                evening.room.closed(now, HOSTILE);
                evening.hostile.closed(now);
                evening.hostile_open = false;
            }
            Op::Forget => {
                if let Some(hostile) = hostile_device {
                    evening.room.forget(hostile);
                }
            }
            Op::PhoneEvent(value) => {
                let event = Up::Tap(value);
                if evening.honest.send(&event).is_ok() {
                    sent_up.push(event);
                }
            }
            Op::PhoneState(value) => {
                let _ = evening.honest.set_state(&Up::Tap(value));
            }
            Op::GameEvent(value) => {
                let event = Down::Note(value);
                if evening.room.send(device, &event).is_ok() {
                    sent_down.push(event);
                }
            }
            Op::GameState(value) => {
                let state = Down::View(value);
                if evening.room.show(device, &state).is_ok() {
                    shown = Some(state);
                }
            }
            Op::Phase(phase) => evening.room.set_phase(phase),
            Op::Open(true) => evening.room.open(),
            Op::Open(false) => evening.room.close(),
            Op::Wait(ms) => evening.now = now.plus(u64::from(ms) * 4),
            Op::Hostile(_) | Op::Tamper { .. } | Op::Replay | Op::HostileClosed => {}
        }
        evening.pump();
    }
    // Whatever happened on the other links, what the honest phone is owed
    // arrives. Thirty events a second are accepted, so this is long enough.
    for _ in 0..96 {
        evening.now = evening.now.plus(250);
        evening.pump();
        if evening.from_honest.len() >= sent_up.len()
            && evening.to_honest.len() >= sent_down.len()
            && shown
                .as_ref()
                .is_none_or(|state| evening.states_to_honest.last() == Some(state))
        {
            break;
        }
    }
    assert_eq!(evening.honest.status(), Status::Connected);
    assert_eq!(evening.from_honest, sent_up);
    assert_eq!(evening.to_honest, sent_down);
    if let Some(state) = shown {
        assert_eq!(evening.states_to_honest.last(), Some(&state));
    }
});
