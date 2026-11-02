//! Reliability, edge by edge: fragments, numbering, resends and the round
//! trip, each found by a mutant that survived the simulated network. The
//! other side is played by hand from forged packets.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers outside #[test] functions: a room that does not answer stops the test"
)]

#[expect(
    dead_code,
    reason = "the forged packets are shared with tests/client_edges.rs, which uses all of them"
)]
mod wire;

use salon_core::{
    Caps, DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind, Limits, LinkId, Ms, Output,
    PROTOCOL, Timing, Token, TvKey,
};
use wire::{Ack, Control, Echo, EventPart, Hello, Packet, StatePart, decode, encode};

const EPOCH: u32 = 3;
const LINK: LinkId = LinkId(1);

/// `Vec<u8>` encodes as its length then its bytes: one byte of length here.
type Room = Host<Vec<u8>, Vec<u8>>;

fn message(bytes: &[u8]) -> Vec<u8> {
    postcard::to_allocvec(bytes).expect("bytes encode")
}

fn joined(limits: Limits) -> (Room, DeviceId) {
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
    room.opened(Ms(0), LINK, Caps::DATAGRAM);
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
    let (events, _) = drain(&mut room, Ms(0));
    match events.as_slice() {
        [DeviceEvent::Arrived { device, .. }] => (room, *device),
        other => panic!("{other:?}"),
    }
}

fn drain(room: &mut Room, now: Ms) -> (Vec<DeviceEvent<Vec<u8>>>, Vec<Packet>) {
    let (mut events, mut packets) = (Vec::new(), Vec::new());
    while let Some(output) = room.poll(now) {
        match output {
            Output::Transmit { bytes, .. } => packets.push(decode(&bytes)),
            Output::Close { .. } => {}
            Output::Device(event) => events.push(event),
        }
    }
    (events, packets)
}

fn events_part(room: &mut Room, now: Ms, parts: Vec<EventPart>) -> Vec<Vec<u8>> {
    let packet = Packet {
        epoch: EPOCH,
        sent: now.0,
        events: parts,
        ..Packet::default()
    };
    room.received(now, LINK, &encode(&packet));
    drain(room, now)
        .0
        .into_iter()
        .filter_map(|event| {
            if let DeviceEvent::Event { event, .. } = event {
                Some(event)
            } else {
                None
            }
        })
        .collect()
}

fn states_part(room: &mut Room, now: Ms, parts: Vec<StatePart>) -> Vec<Vec<u8>> {
    let packet = Packet {
        epoch: EPOCH,
        sent: now.0,
        state: parts,
        ..Packet::default()
    };
    room.received(now, LINK, &encode(&packet));
    drain(room, now)
        .0
        .into_iter()
        .filter_map(|event| {
            if let DeviceEvent::State { state, .. } = event {
                Some(state)
            } else {
                None
            }
        })
        .collect()
}

fn event_part(seq: u32, index: u16, count: u16, data: &[u8]) -> EventPart {
    EventPart {
        seq,
        index,
        count,
        phase: 0,
        at: None,
        data: data.to_vec(),
    }
}

fn state_part(seq: u32, index: u16, count: u16, data: &[u8]) -> StatePart {
    StatePart {
        seq,
        index,
        count,
        data: data.to_vec(),
    }
}

fn garbage(room: &Room, device: DeviceId) -> u32 {
    room.device(device).expect("a device").garbage
}

#[test]
fn fragments_come_together_in_their_order_whatever_the_order_of_arrival() {
    let (mut room, _) = joined(Limits::default());
    let whole = message(&[1, 2, 3, 4, 5]);
    let (head, tail) = whole.split_at(2);
    assert_eq!(
        events_part(&mut room, Ms(1), vec![event_part(0, 1, 2, tail)]),
        Vec::<Vec<u8>>::new()
    );
    // The same piece twice is not two pieces.
    assert_eq!(
        events_part(&mut room, Ms(2), vec![event_part(0, 1, 2, tail)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        events_part(&mut room, Ms(3), vec![event_part(0, 0, 2, head)]),
        vec![vec![1, 2, 3, 4, 5]]
    );

    let (mut room, device) = joined(Limits::default());
    let packet_state = states_part(&mut room, Ms(1), vec![state_part(1, 1, 2, tail)]);
    assert_eq!(packet_state, Vec::<Vec<u8>>::new());
    assert_eq!(
        states_part(&mut room, Ms(2), vec![state_part(1, 0, 2, head)]),
        vec![vec![1, 2, 3, 4, 5]]
    );
    assert_eq!(garbage(&room, device), 0);
}

#[test]
fn nonsense_fragments_are_counted_and_the_largest_legal_count_is_not() {
    let (mut room, device) = joined(Limits::default());
    for (index, count) in [(0, 0), (2, 2), (3, 2), (0, 1_025)] {
        events_part(&mut room, Ms(1), vec![event_part(0, index, count, &[1])]);
    }
    assert_eq!(garbage(&room, device), 4);
    events_part(&mut room, Ms(1), vec![event_part(0, 1_023, 1_024, &[1])]);
    assert_eq!(garbage(&room, device), 4);

    let (mut room, device) = joined(Limits::default());
    for (index, count) in [(0, 0), (2, 2), (3, 2), (0, 1_025)] {
        states_part(&mut room, Ms(1), vec![state_part(1, index, count, &[1])]);
    }
    assert_eq!(garbage(&room, device), 4);
    states_part(&mut room, Ms(1), vec![state_part(1, 1_023, 1_024, &[1])]);
    assert_eq!(garbage(&room, device), 4);
}

#[test]
fn a_message_of_exactly_the_limit_arrives_and_a_longer_one_is_passed_over() {
    let limits = Limits {
        max_up: 8,
        ..Limits::default()
    };
    let (mut room, device) = joined(limits);
    let eight = message(&[1; 7]);
    assert_eq!(eight.len(), 8);
    assert_eq!(
        events_part(&mut room, Ms(1), vec![event_part(0, 0, 1, &eight)]),
        vec![vec![1; 7]]
    );
    assert_eq!(garbage(&room, device), 0);

    // Too long: dropped whole, and the next number is the one after it, so
    // the sender's queue empties.
    let nine = message(&[1; 8]);
    let delivered = events_part(
        &mut room,
        Ms(2),
        vec![event_part(1, 0, 1, &nine), event_part(2, 0, 1, &eight)],
    );
    assert_eq!(delivered, vec![vec![1; 7]]);
    assert_eq!(garbage(&room, device), 1);

    let (mut room, device) = joined(limits);
    assert_eq!(
        states_part(&mut room, Ms(1), vec![state_part(1, 0, 1, &eight)]),
        vec![vec![1; 7]]
    );
    assert_eq!(garbage(&room, device), 0);
    assert_eq!(
        states_part(&mut room, Ms(2), vec![state_part(2, 0, 1, &nine)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(garbage(&room, device), 1);
}

#[test]
fn a_gap_and_a_repeat_deliver_nothing_and_are_not_garbage() {
    let (mut room, device) = joined(Limits::default());
    let one = message(&[1]);
    assert_eq!(
        events_part(&mut room, Ms(1), vec![event_part(1, 0, 1, &one)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        events_part(&mut room, Ms(2), vec![event_part(0, 0, 1, &one)]),
        vec![vec![1]]
    );
    assert_eq!(
        events_part(&mut room, Ms(3), vec![event_part(0, 0, 1, &one)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(garbage(&room, device), 0);
}

/// A resend cut for another link: the pieces of the old cut are gone.
#[test]
fn a_new_cut_of_an_event_starts_again() {
    let (mut room, _) = joined(Limits::default());
    let whole = message(&[1, 2, 3, 4, 5, 6]);
    events_part(&mut room, Ms(1), vec![event_part(0, 0, 3, &whole[..2])]);
    let delivered = events_part(
        &mut room,
        Ms(2),
        vec![
            event_part(0, 0, 2, &whole[..4]),
            event_part(0, 1, 2, &whole[4..]),
        ],
    );
    assert_eq!(delivered, vec![vec![1, 2, 3, 4, 5, 6]]);
}

#[test]
fn an_older_state_never_undoes_a_newer_one() {
    let (mut room, _) = joined(Limits::default());
    let new = message(&[2]);
    let old = message(&[1]);
    // Being built, a newer state wins; a piece of an older one does not
    // disturb it.
    let whole = message(&[7, 8, 9]);
    states_part(&mut room, Ms(1), vec![state_part(5, 0, 2, &whole[..2])]);
    states_part(&mut room, Ms(2), vec![state_part(4, 0, 2, &whole[..2])]);
    assert_eq!(
        states_part(&mut room, Ms(3), vec![state_part(5, 1, 2, &whole[2..])]),
        vec![vec![7, 8, 9]]
    );
    // Applied, an older or equal number is ignored.
    assert_eq!(
        states_part(&mut room, Ms(4), vec![state_part(5, 0, 1, &old)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        states_part(&mut room, Ms(5), vec![state_part(3, 0, 1, &old)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        states_part(&mut room, Ms(6), vec![state_part(6, 0, 1, &new)]),
        vec![vec![2]]
    );
}

/// Only the newest complete state of a batch counts.
#[test]
fn a_state_recut_under_the_same_number_starts_again() {
    let (mut room, _) = joined(Limits::default());
    let whole = message(&[1, 2, 3, 4, 5, 6]);
    states_part(&mut room, Ms(1), vec![state_part(1, 0, 3, &whole[..2])]);
    let delivered = states_part(
        &mut room,
        Ms(2),
        vec![
            state_part(1, 0, 2, &whole[..4]),
            state_part(1, 1, 2, &whole[4..]),
        ],
    );
    assert_eq!(delivered, vec![vec![1, 2, 3, 4, 5, 6]]);
}

/// What the room sends goes again after the resend delay, not before, and not
/// once acknowledged.
#[test]
fn events_go_again_until_acknowledged_and_no_longer() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1]).unwrap();
    room.show(device, &vec![2]).unwrap();
    let (_, packets) = drain(&mut room, Ms(0));
    let parts = |packets: &[Packet]| {
        (
            packets.iter().map(|p| p.events.len()).sum::<usize>(),
            packets.iter().map(|p| p.state.len()).sum::<usize>(),
        )
    };
    assert_eq!(parts(&packets), (1, 1));
    assert_eq!(parts(&drain(&mut room, Ms(249)).1), (0, 0));
    assert_eq!(parts(&drain(&mut room, Ms(250)).1), (1, 1));

    let ack = Packet {
        epoch: EPOCH,
        sent: 260,
        ack: Some(Ack {
            events: 1,
            state: 1,
        }),
        ..Packet::default()
    };
    room.received(Ms(260), LINK, &encode(&ack));
    assert_eq!(parts(&drain(&mut room, Ms(1_000)).1), (0, 0));
}

/// An older acknowledgement says nothing about what came after.
#[test]
fn an_acknowledgement_covers_what_it_names_and_nothing_more() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1]).unwrap();
    room.send(device, &vec![2]).unwrap();
    room.show(device, &vec![3]).unwrap();
    room.show(device, &vec![4]).unwrap();
    drain(&mut room, Ms(0));
    let ack = Packet {
        epoch: EPOCH,
        sent: 10,
        ack: Some(Ack {
            events: 1,
            state: 1,
        }),
        ..Packet::default()
    };
    room.received(Ms(10), LINK, &encode(&ack));
    let (_, packets) = drain(&mut room, Ms(250));
    let seqs: Vec<u32> = packets
        .iter()
        .flat_map(|p| &p.events)
        .map(|p| p.seq)
        .collect();
    assert_eq!(seqs, vec![1]);
    let states: Vec<u32> = packets
        .iter()
        .flat_map(|p| &p.state)
        .map(|p| p.seq)
        .collect();
    assert_eq!(states, vec![2]);
}

/// The resend delay is twice the round trip, between its two bounds.
#[test]
fn the_resend_delay_follows_the_round_trip_within_its_bounds() {
    for (rtt, resend) in [(20, 100), (80, 160), (900, 1_000)] {
        let (mut room, device) = joined(Limits::default());
        let start = Ms(5_000);
        let echo = Packet {
            epoch: EPOCH,
            sent: start.0,
            echo: Some(Echo {
                sent: start.0 - rtt,
                held: 0,
            }),
            ..Packet::default()
        };
        room.received(start, LINK, &encode(&echo));
        assert_eq!(room.device(device).unwrap().rtt, Some(rtt));
        room.send(device, &vec![1]).unwrap();
        assert_eq!(drain(&mut room, start).1.len(), 1);
        let events = |packets: Vec<Packet>| packets.iter().map(|p| p.events.len()).sum::<usize>();
        assert_eq!(events(drain(&mut room, start.plus(resend - 1)).1), 0);
        assert_eq!(events(drain(&mut room, start.plus(resend)).1), 1);
    }
}

#[test]
fn a_nonsense_echo_measures_nothing() {
    let (mut room, device) = joined(Limits::default());
    let echoed = |now: u64, sent: u64, held: u32| {
        encode(&Packet {
            epoch: EPOCH,
            sent: now,
            echo: Some(Echo { sent, held }),
            ..Packet::default()
        })
    };
    // From the future, or held longer than it travelled.
    room.received(Ms(100), LINK, &echoed(100, 200, 0));
    room.received(Ms(100), LINK, &echoed(100, 50, 60));
    assert_eq!(room.device(device).unwrap().rtt, None);
    // Ten seconds is the longest believable round trip.
    room.received(Ms(20_001), LINK, &echoed(20_001, 10_000, 0));
    assert_eq!(room.device(device).unwrap().rtt, None);
    room.received(Ms(20_002), LINK, &echoed(20_002, 10_002, 0));
    assert_eq!(room.device(device).unwrap().rtt, Some(10_000));
}

fn largest_packet(caps: Caps) -> usize {
    let limits = Limits {
        max_down: 256 * 1024,
        ..Limits::default()
    };
    let (mut room, device) = joined(limits);
    room.opened(Ms(1), LinkId(2), caps);
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
    room.received(Ms(1), LinkId(2), &encode(&hello));
    drain(&mut room, Ms(1));
    room.show(device, &vec![1; 200 * 1024]).expect("it fits");
    let mut largest = 0;
    while let Some(output) = room.poll(Ms(2)) {
        if let Output::Transmit { bytes, .. } = output {
            largest = largest.max(bytes.len());
        }
    }
    largest
}

/// A WebSocket carries 16 KiB, and no link more than 64 KiB, whatever it says.
#[test]
fn packets_fill_the_link_up_to_their_limit() {
    let stream = largest_packet(Caps::STREAM);
    assert!((15_000..=16 * 1024).contains(&stream), "{stream}");
    let unbounded = largest_packet(Caps {
        max_packet: 1024 * 1024,
        local: false,
    });
    assert!((60_000..=64 * 1024).contains(&unbounded), "{unbounded}");
}
