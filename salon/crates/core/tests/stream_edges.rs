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
use wire::{Ack, Control, Echo, EventPart, Hello, Packet, Parts, StatePart, decode, encode};

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

/// What arrives past a hole waits for it, once, and a repeat of what was
/// delivered is ignored. None of it is garbage.
#[test]
fn a_gap_waits_for_its_turn_and_a_repeat_delivers_nothing() {
    let (mut room, device) = joined(Limits::default());
    let (one, two) = (message(&[1]), message(&[2]));
    for now in [Ms(1), Ms(2)] {
        assert_eq!(
            events_part(&mut room, now, vec![event_part(1, 0, 1, &two)]),
            Vec::<Vec<u8>>::new()
        );
    }
    assert_eq!(
        events_part(&mut room, Ms(3), vec![event_part(0, 0, 1, &one)]),
        vec![vec![1], vec![2]]
    );
    assert_eq!(
        events_part(&mut room, Ms(4), vec![event_part(0, 0, 1, &one)]),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(garbage(&room, device), 0);
}

/// The ack says what waits past a hole: whole numbers bit by bit, and the
/// first message not whole, fragment by fragment.
#[test]
fn an_acknowledgement_says_what_waits_past_a_hole() {
    let (mut room, _) = joined(Limits::default());
    let one = message(&[1]);
    let parts = vec![
        event_part(1, 0, 1, &one),
        event_part(3, 1, 2, &one),
        event_part(4, 0, 2, &one),
        event_part(63, 0, 1, &one),
    ];
    let ack = answer(&mut room, Ms(1), parts, Vec::new());
    assert_eq!((ack.events, ack.ahead), (0, 1 << 1 | 1 << 63));
    let parts = Parts {
        seq: 3,
        count: 2,
        first: 0,
        more: 1,
    };
    assert_eq!(ack.event_parts, Some(parts));
}

/// Past 64 numbers nothing is kept: the sender goes in its order.
#[test]
fn nothing_is_kept_past_the_last_number_an_acknowledgement_can_say() {
    let (mut room, _) = joined(Limits::default());
    let one = message(&[1]);
    let parts = vec![event_part(64, 0, 1, &one)];
    let ack = answer(&mut room, Ms(1), parts, Vec::new());
    assert_eq!((ack.ahead, ack.event_parts), (0, None));
}

/// A message too big past a hole is dropped at once, and passed over when its
/// turn comes: the sender's queue empties all the same.
#[test]
fn a_message_too_big_past_a_hole_is_passed_over_in_its_turn() {
    let limits = Limits {
        max_up: 8,
        ..Limits::default()
    };
    let (mut room, device) = joined(limits);
    let (fits, long) = (message(&[1; 7]), message(&[1; 8]));
    let parts = vec![event_part(1, 0, 1, &long)];
    let ack = answer(&mut room, Ms(1), parts, Vec::new());
    assert_eq!((ack.events, ack.ahead), (0, 1 << 1));
    assert_eq!(garbage(&room, device), 1);
    assert_eq!(
        events_part(&mut room, Ms(2), vec![event_part(0, 0, 1, &fits)]),
        vec![vec![1; 7]]
    );
    let parts = vec![event_part(2, 0, 1, &fits)];
    let ack = answer(&mut room, Ms(3), parts, Vec::new());
    assert_eq!((ack.events, ack.ahead), (3, 0));
}

/// What waits past a hole belongs to the page that sent it: another page's
/// numbers mean something else.
#[test]
fn a_new_page_inherits_nothing_of_what_waited_past_a_hole() {
    let (mut room, _) = joined(Limits::default());
    let (old, new) = (message(&[1]), message(&[2]));
    events_part(&mut room, Ms(1), vec![event_part(1, 0, 1, &old)]);
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
    room.opened(Ms(2), LinkId(2), Caps::DATAGRAM);
    room.received(Ms(2), LinkId(2), &encode(&hello));
    drain(&mut room, Ms(2));
    let packet = Packet {
        epoch: EPOCH,
        sent: 3,
        events: vec![event_part(0, 0, 1, &new)],
        ..Packet::default()
    };
    room.received(Ms(3), LinkId(2), &encode(&packet));
    let heard = drain(&mut room, Ms(3)).0;
    assert!(
        matches!(heard.as_slice(), [DeviceEvent::Event { event, .. }] if *event == vec![2]),
        "{heard:?}"
    );
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
            ..Ack::default()
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
            ..Ack::default()
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

/// Takes these fragments and says what the room answers: the bytes are still
/// to drain in the helpers above, which keep what they hear to themselves.
fn answer(room: &mut Room, now: Ms, events: Vec<EventPart>, state: Vec<StatePart>) -> Ack {
    let packet = Packet {
        epoch: EPOCH,
        sent: now.0,
        events,
        state,
        ..Packet::default()
    };
    room.received(now, LINK, &encode(&packet));
    drain(room, now)
        .1
        .iter()
        .find_map(|packet| packet.ack)
        .expect("an acknowledgement")
}

/// A message that comes in pieces is acknowledged piece by piece: the first
/// missing one, then the 64 past it. Nothing past them is said, not even
/// wrongly: bit 63 is the last one.
#[test]
fn an_acknowledgement_names_the_fragments_that_arrived() {
    let (mut room, _) = joined(Limits::default());
    let pieces = [0, 1, 3, 66, 67].map(|index| event_part(0, index, 100, &[1]));
    let ack = answer(&mut room, Ms(1), pieces.to_vec(), Vec::new());
    let parts = Parts {
        seq: 0,
        count: 100,
        first: 2,
        more: 1 | 1 << 63,
    };
    assert_eq!(ack.event_parts, Some(parts));

    let (mut room, _) = joined(Limits::default());
    let pieces = [0, 2].map(|index| state_part(4, index, 3, &[1]));
    let ack = answer(&mut room, Ms(1), Vec::new(), pieces.to_vec());
    let parts = Parts {
        seq: 4,
        count: 3,
        first: 1,
        more: 1,
    };
    assert_eq!((ack.event_parts, ack.state_parts), (None, Some(parts)));
}

/// The number of a message and of one of its fragments.
type Fragments = Vec<(u32, u16)>;

/// The fragments of events, then of states, sent to a device.
fn sent(packets: &[Packet]) -> (Fragments, Fragments) {
    let events = packets.iter().flat_map(|packet| &packet.events);
    let states = packets.iter().flat_map(|packet| &packet.state);
    (
        events.map(|part| (part.seq, part.index)).collect(),
        states.map(|part| (part.seq, part.index)).collect(),
    )
}

/// A resend carries what is missing and nothing else, so a pipe that passes
/// the head of each burst ends up passing the tail.
#[test]
fn a_resend_carries_only_the_fragments_that_did_not_arrive() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 2_500]).unwrap();
    room.show(device, &vec![2; 2_500]).unwrap();
    let (events, states) = sent(&drain(&mut room, Ms(0)).1);
    assert_eq!(events.len(), 3);
    assert_eq!(states.len(), 3);
    let ack = Packet {
        epoch: EPOCH,
        sent: 10,
        ack: Some(Ack {
            event_parts: Some(Parts {
                seq: 0,
                count: 3,
                first: 1,
                more: 1,
            }),
            state_parts: Some(Parts {
                seq: 1,
                count: 3,
                first: 2,
                more: 0,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    };
    room.received(Ms(10), LINK, &encode(&ack));
    assert_eq!(
        sent(&drain(&mut room, Ms(250)).1),
        (vec![(0, 1)], vec![(1, 2)])
    );
}

/// A fragment said to have arrived of a message the sender has moved past
/// means nothing, and neither does a fragment that does not exist.
#[test]
fn an_acknowledgement_of_other_fragments_changes_nothing() {
    let (mut room, device) = joined(Limits::default());
    room.show(device, &vec![2; 2_500]).unwrap();
    room.show(device, &vec![3; 2_500]).unwrap();
    let (_, states) = sent(&drain(&mut room, Ms(0)).1);
    let ack = Packet {
        epoch: EPOCH,
        sent: 10,
        ack: Some(Ack {
            state_parts: Some(Parts {
                seq: 1,
                count: 3,
                first: u16::MAX,
                more: u64::MAX,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    };
    room.received(Ms(10), LINK, &encode(&ack));
    assert_eq!(sent(&drain(&mut room, Ms(250)).1).1, states);
}

/// An event over budget stays here, whole: its sender was told it arrived, and
/// does not send it again. The next packet lets it out, and it is acknowledged.
#[test]
fn an_event_over_budget_goes_with_a_later_packet_of_any_kind() {
    let limits = Limits {
        events_per_s: 10,
        events_burst: 1,
        ..Limits::default()
    };
    let (mut room, _) = joined(limits);
    let one = message(&[1]);
    let parts = vec![event_part(0, 0, 1, &one), event_part(1, 0, 1, &one)];
    let first = answer(&mut room, Ms(1), parts, Vec::new());
    assert_eq!(first.events, 1);
    let held = answer(
        &mut room,
        Ms(2),
        vec![event_part(1, 0, 1, &one)],
        Vec::new(),
    );
    assert_eq!((held.events, held.ahead), (1, 1));
    let later = answer(&mut room, Ms(1_000), Vec::new(), Vec::new());
    assert_eq!((later.events, later.ahead), (2, 0));
}

/// An ack written after the fragments left, which says what lies past the
/// first one arrived.
fn passed_over(room: &mut Room, now: u64, ahead: u64) {
    let ack = Packet {
        epoch: EPOCH,
        sent: now,
        echo: Some(Echo { sent: now, held: 0 }),
        ack: Some(Ack {
            ahead,
            ..Ack::default()
        }),
        ..Packet::default()
    };
    room.received(Ms(now), LINK, &encode(&ack));
}

/// Three acks that pass over a fragment send it again without its timer, and
/// nothing else with it. Two may be a packet that is late.
#[test]
fn a_fragment_passed_over_three_times_goes_without_its_timer() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1]).unwrap();
    room.send(device, &vec![2]).unwrap();
    room.send(device, &vec![3]).unwrap();
    assert_eq!(sent(&drain(&mut room, Ms(0)).1).0.len(), 3);
    for now in [1, 2] {
        passed_over(&mut room, now, 1 << 1);
        assert_eq!(sent(&drain(&mut room, Ms(now)).1).0, Vec::new());
    }
    passed_over(&mut room, 3, 1 << 1);
    assert_eq!(sent(&drain(&mut room, Ms(3)).1).0, vec![(0, 0)]);
}

/// A fragment that left after an ack was written is not in it: the echo says
/// so, and the same report of a hole does not send it a second time.
#[test]
fn an_ack_written_before_a_resend_does_not_count_against_it() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1]).unwrap();
    room.send(device, &vec![2]).unwrap();
    drain(&mut room, Ms(0));
    for now in 1..=3 {
        passed_over(&mut room, now, 1 << 1);
    }
    assert_eq!(sent(&drain(&mut room, Ms(3)).1).0, vec![(0, 0)]);
    for now in 4..=9 {
        let late = Packet {
            epoch: EPOCH,
            sent: now,
            echo: Some(Echo { sent: 2, held: 0 }),
            ack: Some(Ack {
                ahead: 1 << 1,
                ..Ack::default()
            }),
            ..Packet::default()
        };
        room.received(Ms(now), LINK, &encode(&late));
        assert_eq!(sent(&drain(&mut room, Ms(now)).1).0, Vec::new());
    }
}

/// A sender stays where its receiver keeps what it sends.
#[test]
fn a_sender_stays_within_what_its_receiver_keeps() {
    let (mut room, device) = joined(Limits::default());
    for n in 0..100 {
        room.send(device, &vec![n]).unwrap();
    }
    let (first, _) = sent(&drain(&mut room, Ms(0)).1);
    assert_eq!(first.last(), Some(&(63, 0)));
    // What may not leave yet is not a reason to wake up at once.
    assert!(room.next_deadline() > Some(Ms(0)));
}

/// How many fragments of events leave now.
fn leaving(room: &mut Room, now: Ms) -> usize {
    sent(&drain(room, now).1).0.len()
}

/// What says every fragment below `first` of message 0 arrived.
fn fragments_below(first: u16, count: u16, now: u64) -> Vec<u8> {
    encode(&Packet {
        epoch: EPOCH,
        sent: now,
        ack: Some(Ack {
            event_parts: Some(Parts {
                seq: 0,
                count,
                first,
                more: 0,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    })
}

/// A big message does not leave at once: ten packets, then what the acks say
/// has passed. Each one that arrived makes room for two.
#[test]
fn a_big_message_leaves_by_the_window() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 12);
    assert_eq!(leaving(&mut room, Ms(1)), 0);
    assert!(room.next_deadline() > Some(Ms(1)), "nothing to wake up for");
    room.received(Ms(2), LINK, &fragments_below(12, 64, 2));
    assert_eq!(leaving(&mut room, Ms(2)), 24);
    room.received(Ms(3), LINK, &fragments_below(36, 64, 3));
    assert_eq!(leaving(&mut room, Ms(3)), 28);
}

/// Each timer that runs out with nothing heard doubles the next wait, up to
/// its limit, and an ack that brings news starts over.
#[test]
fn each_timer_that_runs_out_doubles_the_next_wait() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 2_500]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 3);
    for (before, at) in [(249, 250), (749, 750), (1_749, 1_750), (2_749, 2_750)] {
        assert_eq!(leaving(&mut room, Ms(before)), 0, "{before}");
        assert!(leaving(&mut room, Ms(at)) > 0, "{at}");
    }
    room.received(Ms(2_751), LINK, &fragments_below(1, 3, 2_751));
    assert_eq!(leaving(&mut room, Ms(2_999)), 0);
    assert!(leaving(&mut room, Ms(3_000)) > 0, "back to 250 ms");
}

/// The latest position of a stick does not wait behind a queue of events.
#[test]
fn a_state_of_one_packet_goes_whatever_the_window() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 12);
    room.show(device, &vec![2]).unwrap();
    assert_eq!(sent(&drain(&mut room, Ms(1)).1), (vec![], vec![(1, 0)]));
}

/// A room with one device on a stream.
fn on_a_stream() -> (Room, DeviceId) {
    let (mut room, device) = joined(Limits::default());
    room.opened(Ms(1), LinkId(2), Caps::STREAM);
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
    (room, device)
}

/// No timer on a stream: only what comes back says a packet was dropped, and
/// an echo of something that left after it is what proves it.
#[test]
fn a_stream_sends_again_what_an_echo_proves_lost_and_nothing_else() {
    let (mut room, device) = on_a_stream();
    room.send(device, &vec![1]).unwrap();
    room.send(device, &vec![2]).unwrap();
    assert_eq!(leaving(&mut room, Ms(10)), 2);
    assert_eq!(leaving(&mut room, Ms(5_000)), 0);
    let ack = |sent: u64, echoed: u64| {
        encode(&Packet {
            epoch: EPOCH,
            sent,
            echo: Some(Echo {
                sent: echoed,
                held: 0,
            }),
            ack: Some(Ack {
                ahead: 1 << 1,
                ..Ack::default()
            }),
            ..Packet::default()
        })
    };
    // The same millisecond proves nothing: both left then.
    room.received(Ms(5_001), LinkId(2), &ack(5_001, 10));
    assert_eq!(leaving(&mut room, Ms(5_001)), 0);
    room.received(Ms(5_002), LinkId(2), &ack(5_002, 11));
    assert_eq!(sent(&drain(&mut room, Ms(5_002)).1).0, vec![(0, 0)]);
}

/// A state too big for the room is dropped, and passed over like an event:
/// its sender stops sending it, and what follows is heard.
#[test]
fn a_state_too_big_is_passed_over() {
    let limits = Limits {
        max_up: 8,
        ..Limits::default()
    };
    let (mut room, _) = joined(limits);
    let long = vec![state_part(1, 0, 1, &message(&[1; 8]))];
    let ack = answer(&mut room, Ms(1), Vec::new(), long);
    assert_eq!(ack.state, 1);
    assert_eq!(
        states_part(&mut room, Ms(2), vec![state_part(2, 0, 1, &message(&[2]))]),
        vec![vec![2]]
    );
}

/// What arrived under the cut of another link names other fragments, and the
/// ack that says so can leave before the first fragment of the new cut lands.
/// Read as this cut's, it would stop the fragments that are still missing.
#[test]
fn an_ack_of_another_cut_says_nothing() {
    let (mut room, device) = on_a_stream();
    room.send(device, &vec![1; 40_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(10)), 3);
    let of_the_old_cut = encode(&Packet {
        epoch: EPOCH,
        sent: 20,
        echo: Some(Echo { sent: 5, held: 0 }),
        ack: Some(Ack {
            event_parts: Some(Parts {
                seq: 0,
                count: 43,
                first: 30,
                more: 0,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    });
    room.received(Ms(20), LinkId(2), &of_the_old_cut);
    let proof = encode(&Packet {
        epoch: EPOCH,
        sent: 21,
        echo: Some(Echo { sent: 11, held: 0 }),
        ack: Some(Ack::default()),
        ..Packet::default()
    });
    room.received(Ms(21), LinkId(2), &proof);
    assert_eq!(leaving(&mut room, Ms(21)), 3, "they are still owed");
}

/// A stream has no timer, so what an ack cannot name must still go again: an
/// old cut, or an older state, is all the receiver holds, and it would say so
/// for ever while what replaced it never lands.
#[test]
fn a_stream_does_not_wait_behind_what_the_other_side_kept_of_the_past() {
    let (mut room, device) = on_a_stream();
    room.send(device, &vec![1; 40_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(10)), 3);
    let old_cut = encode(&Packet {
        epoch: EPOCH,
        sent: 20,
        echo: Some(Echo { sent: 11, held: 0 }),
        ack: Some(Ack {
            event_parts: Some(Parts {
                seq: 0,
                count: 43,
                first: 30,
                more: 0,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    });
    room.received(Ms(20), LinkId(2), &old_cut);
    assert_eq!(leaving(&mut room, Ms(20)), 3);

    let (mut room, device) = on_a_stream();
    room.show(device, &vec![1; 40_000]).unwrap();
    room.show(device, &vec![2]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(10)), vec![2]);
    let older = encode(&Packet {
        epoch: EPOCH,
        sent: 20,
        echo: Some(Echo { sent: 11, held: 0 }),
        ack: Some(Ack {
            state_parts: Some(Parts {
                seq: 1,
                count: 3,
                first: 1,
                more: 0,
            }),
            ..Ack::default()
        }),
        ..Packet::default()
    });
    room.received(Ms(20), LinkId(2), &older);
    assert_eq!(states_leaving(&mut room, Ms(20)), vec![2]);
}

/// The window counts what a fragment weighs, not the size of the cut: small
/// events on a stream weigh a few bytes each, not 16 KiB.
#[test]
fn small_fragments_weigh_what_they_carry() {
    let (mut room, device) = on_a_stream();
    for _ in 0..10 {
        room.send(device, &vec![1]).unwrap();
    }
    assert_eq!(leaving(&mut room, Ms(10)), 10);
    for _ in 0..10 {
        room.send(device, &vec![2]).unwrap();
    }
    assert_eq!(leaving(&mut room, Ms(11)), 10);
}

/// A fragment known lost has left what is on its way, and nothing else may
/// free the window: on a stream, what follows it waits for it to arrive, and
/// no timer would ever open it.
#[test]
fn a_fragment_known_lost_goes_whatever_the_window() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 12);
    let ack = |sent: u64, echoed: u64| {
        encode(&Packet {
            epoch: EPOCH,
            sent,
            echo: Some(Echo {
                sent: echoed,
                held: 0,
            }),
            ack: Some(Ack {
                event_parts: Some(Parts {
                    seq: 0,
                    count: 64,
                    first: 5,
                    more: 0b11_1111,
                }),
                ..Ack::default()
            }),
            ..Packet::default()
        })
    };
    room.received(Ms(2), LINK, &ack(2, 1));
    assert!(leaving(&mut room, Ms(2)) > 0, "what the ack made room for");
    room.received(Ms(3), LINK, &ack(3, 1));
    assert_eq!(leaving(&mut room, Ms(3)), 0, "the window is full");
    room.received(Ms(4), LINK, &ack(4, 1));
    assert_eq!(sent(&drain(&mut room, Ms(4)).1).0, vec![(0, 5)]);
}

/// The states that left, by number.
fn states_leaving(room: &mut Room, now: Ms) -> Vec<u32> {
    let mut numbers: Vec<u32> = sent(&drain(room, now).1)
        .1
        .iter()
        .map(|(seq, _)| *seq)
        .collect();
    numbers.dedup();
    numbers
}

fn applied(state: u32, now: u64) -> Vec<u8> {
    encode(&Packet {
        epoch: EPOCH,
        sent: now,
        ack: Some(Ack {
            state,
            ..Ack::default()
        }),
        ..Packet::default()
    })
}

/// A state of several fragments on its way finishes, and only the latest of
/// those shown meanwhile follows it.
#[test]
fn a_state_on_its_way_finishes_and_the_latest_follows() {
    let (mut room, device) = joined(Limits::default());
    room.show(device, &vec![1; 2_500]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(0)), vec![1]);
    room.show(device, &vec![2; 2_500]).unwrap();
    room.show(device, &vec![3; 2_500]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(250)), vec![1]);
    room.received(Ms(251), LINK, &applied(1, 251));
    let (_, packets) = drain(&mut room, Ms(251));
    assert_eq!(sent(&packets).1, vec![(2, 0), (2, 1), (2, 2)]);
    // Past the first fragment, which starts with the message's length.
    let tail = packets
        .iter()
        .flat_map(|packet| &packet.state)
        .filter(|part| part.index > 0);
    assert!(tail.flat_map(|part| &part.data).all(|byte| *byte == 3));
}

/// Nothing is lost by replacing a state that fits one packet, or one that has
/// not left yet.
#[test]
fn a_state_with_nothing_on_its_way_is_replaced() {
    let (mut room, device) = joined(Limits::default());
    room.show(device, &vec![1]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(0)), vec![1]);
    room.show(device, &vec![2]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(1)), vec![2]);

    let (mut room, device) = joined(Limits::default());
    room.show(device, &vec![1; 2_500]).unwrap();
    room.show(device, &vec![2; 2_500]).unwrap();
    assert_eq!(states_leaving(&mut room, Ms(0)), vec![2]);
}

/// A new link owes nothing to what was on its way on the old one: the latest
/// goes first.
#[test]
fn a_new_link_starts_from_the_latest_state() {
    let (mut room, device) = joined(Limits::default());
    room.show(device, &vec![1; 2_500]).unwrap();
    drain(&mut room, Ms(0));
    room.show(device, &vec![2; 2_500]).unwrap();
    room.opened(Ms(1), LinkId(2), Caps::DATAGRAM);
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
    let parts = drain(&mut room, Ms(1)).1;
    let numbers: Vec<u32> = parts
        .iter()
        .flat_map(|packet| &packet.state)
        .map(|part| part.seq)
        .collect();
    assert_eq!(numbers.first(), Some(&2));
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

/// A state of one packet passes the window, and opens it for nothing else: what
/// follows it in the burst still waits for the acks.
#[test]
fn a_state_of_one_packet_opens_the_window_for_nothing_else() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    room.show(device, &vec![2]).unwrap();
    let (events, states) = sent(&drain(&mut room, Ms(0)).1);
    assert_eq!(events.len(), 12);
    assert_eq!(states, vec![(1, 0)]);
}

/// The timers of a big message run out `times` times with nothing heard, and
/// the phone stays there. Says when that ended.
fn deaf(room: &mut Room, times: u32) -> u64 {
    let (mut now, mut expired) = (0, 0);
    while expired < times {
        now += 1_000;
        room.received(Ms(now), LINK, &fragments_below(0, 64, now));
        if leaving(room, Ms(now)) > 0 {
            expired += 1;
        }
    }
    now
}

/// What a long deafness leaves of a window is its floor. A window under it
/// would not read an ack as a few bytes more: at 2 bytes, one fragment heard
/// asks for all that fits.
#[test]
fn a_window_that_fell_far_does_not_jump_at_the_first_ack() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 12);
    let now = deaf(&mut room, 12);
    room.received(Ms(now + 1), LINK, &fragments_below(1, 64, now + 1));
    let out = leaving(&mut room, Ms(now + 1));
    assert!(out <= 3, "{out} fragments left at once");
}

/// A window halved down to nothing stays at nothing unless it has a floor:
/// each ack adds a share of it, and the link stays at two packets for good.
#[test]
fn a_window_that_fell_to_its_floor_grows_again() {
    let (mut room, device) = joined(Limits::default());
    room.send(device, &vec![1; 60_000]).unwrap();
    room.send(device, &vec![2; 60_000]).unwrap();
    assert_eq!(leaving(&mut room, Ms(0)), 12);
    let mut now = deaf(&mut room, 16);
    // From here the link is healthy again: everything sent so far arrives.
    let (mut first, mut most) = (0, 0);
    for _ in 0..40 {
        now += 300;
        room.received(Ms(now), LINK, &fragments_below(first, 64, now));
        let out = sent(&drain(&mut room, Ms(now)).1).0;
        most = out.len().max(most);
        first = out
            .iter()
            .filter(|(seq, _)| *seq == 0)
            .map(|(_, index)| index + 1)
            .max()
            .unwrap_or(first);
        if first == 64 {
            break;
        }
    }
    assert!(most > 3, "the window never left its floor: {most}");
}

/// Enough events on a stream to keep it busy, each a whole fragment.
fn busy_stream() -> (Room, DeviceId) {
    let (mut room, device) = on_a_stream();
    for _ in 0..30 {
        room.send(device, &vec![1; 15_000]).unwrap();
    }
    (room, device)
}

/// What comes back when the event `events` was dropped and the `after` that
/// follow it came: an echo of something that left after it proves it.
fn hole_at(events: u32, after: u32, sent: u64, echoed: u64) -> Vec<u8> {
    encode(&Packet {
        epoch: EPOCH,
        sent,
        echo: Some(Echo {
            sent: echoed,
            held: 0,
        }),
        ack: Some(Ack {
            events,
            ahead: ((1 << after) - 1) << 1,
            ..Ack::default()
        }),
        ..Packet::default()
    })
}

fn all_below(events: u32, sent: u64) -> Vec<u8> {
    encode(&Packet {
        epoch: EPOCH,
        sent,
        ack: Some(Ack {
            events,
            ..Ack::default()
        }),
        ..Packet::default()
    })
}

/// Each loss halves the window of a stream too, however long it lives. A
/// stream's fragments wait for ever: that is no reason for its window to take
/// a single loss in its whole life.
#[test]
fn a_stream_halves_its_window_at_each_loss() {
    let (mut room, _) = busy_stream();
    assert_eq!(leaving(&mut room, Ms(10)), 3);
    room.received(Ms(11), LinkId(2), &all_below(3, 11));
    assert_eq!(leaving(&mut room, Ms(11)), 4);
    room.received(Ms(12), LinkId(2), &all_below(7, 12));
    assert_eq!(leaving(&mut room, Ms(12)), 7, "96 KiB");
    room.received(Ms(1_000), LinkId(2), &hole_at(7, 6, 1_000, 500));
    assert_eq!(leaving(&mut room, Ms(1_000)), 4, "48 KiB, the lost one first");
    room.received(Ms(3_000), LinkId(2), &hole_at(14, 2, 3_000, 2_500));
    assert_eq!(leaving(&mut room, Ms(3_000)), 3, "the floor of two packets");
}
