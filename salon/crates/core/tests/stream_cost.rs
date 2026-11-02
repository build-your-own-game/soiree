//! What one packet costs the room, counted and not timed. A phone may send
//! [`Limits::packets_burst`] packets at once: what each one costs falls on the
//! thread of the whole room (E3). The allocator is the counter: same packet,
//! same number, on any machine and under any load.

#![expect(
    unsafe_code,
    reason = "a global allocator is unsafe to write: this one only counts, then calls the system's"
)]
#![expect(
    clippy::expect_used,
    reason = "test helpers outside #[test] functions: a room that does not answer stops the test"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]

#[expect(
    dead_code,
    reason = "the forged packets are shared with tests/client_edges.rs, which uses all of them"
)]
mod wire;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind,
    Limits, LinkId, Ms, Output, PROTOCOL, Timing, Token, TvKey,
};
use wire::{Control, EventPart, Hello, Packet, StatePart, encode};

thread_local! {
    /// On this thread: the tests of this file run side by side.
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: every call goes to the system allocator, unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.with(|bytes| bytes.set(bytes.get().saturating_add(layout.size())));
        // SAFETY: the caller's obligations are the system's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller's obligations are the system's.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const EPOCH: u32 = 3;
const LINK: LinkId = LinkId(1);
/// What a full packet's bytes may cost in memory, all allocations added up:
/// reading it (a fragment takes 56 bytes in memory and may take 7 on the
/// wire) and storing what it carries.
const COST_PER_BYTE: usize = 16;
/// The largest packet a link may carry.
const MAX_PACKET: usize = 64 * 1024;

type Room = Host<Vec<u8>, Vec<u8>>;

fn joined(limits: Limits) -> Room {
    let mut room = Room::new(
        Ms(0),
        HostConfig {
            game: 7,
            epoch: Epoch(EPOCH),
            tv_key: TvKey([9; 16]),
            timing: Timing::default(),
            limits,
            known: Vec::new(),
        },
    );
    room.opened(Ms(0), LINK, Caps::STREAM);
    let hello = Packet {
        control: Some(Control::Hello(Hello {
            protocol: PROTOCOL,
            game: 7,
            kind: Kind::Phone,
            token: Token([1; 16]),
            name: None,
            key: None,
            resume: None,
        })),
        ..Packet::default()
    };
    room.received(Ms(0), LINK, &encode(&hello));
    while room.poll(Ms(0)).is_some() {}
    room
}

fn device(room: &Room) -> DeviceId {
    room.devices().next().expect("a device").0
}

/// The count changes at each fragment: every one announces a new message of
/// the largest size, under the number the host is waiting for.
fn flapping_events(parts: usize) -> Packet {
    let events = (0..parts)
        .map(|part| EventPart {
            seq: 0,
            index: 0,
            count: if part % 2 == 0 { 1024 } else { 1023 },
            phase: 0,
            at: None,
            data: Vec::new(),
        })
        .collect();
    Packet {
        epoch: EPOCH,
        events,
        ..Packet::default()
    }
}

fn flapping_state(parts: usize) -> Packet {
    let state = (0..parts)
        .map(|part| StatePart {
            seq: 1,
            index: 0,
            count: if part % 2 == 0 { 1024 } else { 1023 },
            data: Vec::new(),
        })
        .collect();
    Packet {
        epoch: EPOCH,
        state,
        ..Packet::default()
    }
}

/// Every fragment of one message, again and again: nothing ever completes it.
fn repeated_events(parts: usize) -> Packet {
    let events = (0..parts)
        .map(|part| EventPart {
            seq: 0,
            index: u16::try_from(part % 1023).expect("below 1023"),
            count: 1024,
            phase: 0,
            at: None,
            data: Vec::new(),
        })
        .collect();
    Packet {
        epoch: EPOCH,
        events,
        ..Packet::default()
    }
}

/// The most of `make` that fits a packet of 64 KiB.
fn largest(make: fn(usize) -> Packet) -> Vec<u8> {
    let (mut fits, mut too_many) = (1, MAX_PACKET);
    while fits + 1 < too_many {
        let middle = usize::midpoint(fits, too_many);
        if encode(&make(middle)).len() <= MAX_PACKET {
            fits = middle;
        } else {
            too_many = middle;
        }
    }
    encode(&make(fits))
}

fn allocated_by(act: impl FnOnce()) -> usize {
    let before = ALLOCATED.with(Cell::get);
    act();
    ALLOCATED.with(Cell::get) - before
}

/// E3: 9 000 to 13 000 empty fragments fit one valid packet. Each one, by
/// announcing a new message, took 1 024 slots: 480 packets kept the thread
/// of the whole room for three seconds.
#[test]
fn a_packet_costs_what_it_carries_not_what_it_announces() {
    let shapes = [
        ("events", flapping_events as fn(usize) -> Packet),
        ("state", flapping_state),
        ("one message", repeated_events),
    ];
    for (name, make) in shapes {
        let bytes = largest(make);
        let mut room = joined(Limits::default());
        let cost = allocated_by(|| room.received(Ms(1_000), LINK, &bytes));
        assert!(
            cost <= bytes.len() * COST_PER_BYTE,
            "{name}: {} bytes on the wire allocated {cost}",
            bytes.len(),
        );
    }
}

/// What no sender can do is not read: the parts of a packet each take room in
/// it, whatever they carry.
#[test]
fn a_packet_with_more_parts_than_a_sender_can_fit_is_garbage() {
    /// `Budget`: 112 bytes for the packet, then 40 for each part.
    const MOST: usize = (MAX_PACKET - 112) / 40;
    let single = |parts: usize| Packet {
        epoch: EPOCH,
        events: (0..parts)
            .map(|part| EventPart {
                seq: u32::try_from(part).expect("a small number"),
                index: 0,
                count: 1,
                phase: 0,
                at: None,
                data: vec![0],
            })
            .collect(),
        ..Packet::default()
    };
    let generous = Limits {
        events_per_s: 100_000,
        events_burst: 100_000,
        ..Limits::default()
    };
    let mut room = joined(generous);
    let id = device(&room);
    room.received(Ms(1), LINK, &encode(&single(MOST + 1)));
    assert_eq!(room.device(id).expect("a device").garbage, 1);
    let mut heard = 0;
    while let Some(output) = room.poll(Ms(1)) {
        heard += usize::from(matches!(output, Output::Device(DeviceEvent::Event { .. })));
    }
    assert_eq!(heard, 0);
    room.received(Ms(2), LINK, &encode(&single(MOST)));
    assert_eq!(room.device(id).expect("a device").garbage, 1);
    while let Some(output) = room.poll(Ms(2)) {
        heard += usize::from(matches!(output, Output::Device(DeviceEvent::Event { .. })));
    }
    assert_eq!(heard, MOST);
}

/// The bound above is what a real sender reaches on the largest link: it
/// never turns its own packets into garbage.
#[test]
fn the_fullest_packet_a_client_sends_still_reads() {
    const EVENTS: usize = 2_000;
    let limits = Limits {
        max_pending: EVENTS,
        events_per_s: 100_000,
        events_burst: 100_000,
        ..Limits::default()
    };
    let caps = Caps {
        max_packet: MAX_PACKET,
        local: false,
    };
    let mut phone: Client<(), ()> = Client::new(
        Ms(0),
        ClientConfig {
            game: 7,
            kind: Kind::Phone,
            token: Token([2; 16]),
            name: None,
            tv_key: None,
            seed: 1,
            timing: Timing::default(),
            limits,
        },
    );
    let mut room: Host<(), ()> = Host::new(
        Ms(0),
        HostConfig {
            game: 7,
            epoch: Epoch(EPOCH),
            tv_key: TvKey([9; 16]),
            timing: Timing::default(),
            limits,
            known: Vec::new(),
        },
    );
    assert_eq!(phone.poll(Ms(0)), Some(ClientOutput::Connect));
    phone.opened(Ms(0), caps);
    room.opened(Ms(0), LINK, caps);
    let mut heard = 0;
    for now in (0..40).map(Ms) {
        while let Some(output) = phone.poll(now) {
            if let ClientOutput::Transmit(bytes) = output {
                room.received(now, LINK, &bytes);
            }
        }
        while let Some(output) = room.poll(now) {
            match output {
                Output::Transmit { bytes, .. } => phone.received(now, &bytes),
                Output::Device(DeviceEvent::Event { .. }) => heard += 1,
                Output::Device(_) | Output::Close { .. } => {}
            }
        }
        if now == Ms(1) {
            for _ in 0..EVENTS {
                phone.send(&()).expect("room for it");
            }
        }
    }
    assert_eq!(heard, EVENTS);
    let id = room.devices().next().expect("a device").0;
    assert_eq!(room.device(id).expect("a device").garbage, 0);
}

/// What waits past a hole has a bound in fragments too: a phone that fills
/// the 64 numbers it may hold with empty ones is not given 65,536 of them.
#[test]
fn what_waits_past_a_hole_is_bounded_in_fragments() {
    let parts = |seq: u32| {
        encode(&Packet {
            epoch: EPOCH,
            events: (0..1024)
                .map(|index| EventPart {
                    seq,
                    index,
                    count: 1024,
                    phase: 0,
                    at: None,
                    data: Vec::new(),
                })
                .collect(),
            ..Packet::default()
        })
    };
    let mut room = joined(Limits::default());
    let first = parts(1);
    let kept = allocated_by(|| room.received(Ms(1), LINK, &first));
    let mut turned_away = 0;
    for seq in 2..64 {
        let next = parts(seq);
        turned_away = allocated_by(|| room.received(Ms(1), LINK, &next)).max(turned_away);
    }
    // Reading a packet costs half of what keeping it does.
    assert!(turned_away < kept / 4 * 3, "{turned_away} against {kept}");
}

/// The room asks this of each device at each turn of its loop: it costs no
/// allocation, however much waits.
#[test]
fn the_next_deadline_allocates_nothing() {
    let mut room = joined(Limits::default());
    let id = device(&room);
    for number in 0..256_u32 {
        room.send(id, &vec![0; 40_000 + number as usize])
            .expect("room");
    }
    while room.poll(Ms(1)).is_some() {}
    let spent = allocated_by(|| {
        assert!(room.next_deadline().is_some());
    });
    assert_eq!(spent, 0);
}
