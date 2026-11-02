//! The room's side, edge by edge, each found by a mutant that survived the
//! simulated network: links are opened by hand and devices are played from
//! forged packets, so every limit can be touched from both sides.

#![expect(
    clippy::panic,
    reason = "test helper outside #[test] functions: a phone that does not join stops the test"
)]

#[expect(
    dead_code,
    reason = "the forged packets are shared with tests/client_edges.rs, which uses all of them"
)]
mod wire;

use salon_core::{
    Caps, DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind, Known, Limits, LinkId, Ms, Name,
    Output, PROTOCOL, Presence, Refusal, SendError, Timing, Token, TvKey, Why,
};
use wire::{Ack, Control, Echo, Hello, Packet, decode, encode, event, state};

const GAME: u32 = 7;
const EPOCH: u32 = 3;
const KEY: TvKey = TvKey([9; 16]);
const LINK: LinkId = LinkId(1);

type Room = Host<u32, Vec<u8>>;

fn config(limits: Limits, known: Vec<Known>) -> HostConfig {
    HostConfig {
        game: GAME,
        epoch: Epoch(EPOCH),
        tv_key: KEY,
        timing: Timing::default(),
        limits,
        known,
    }
}

fn room() -> Room {
    Room::new(Ms(0), config(Limits::default(), Vec::new()))
}

fn hello(token: u8) -> Hello {
    Hello {
        protocol: PROTOCOL,
        game: GAME,
        kind: Kind::Phone,
        token: Token([token; 16]),
        name: None,
        key: None,
        resume: None,
    }
}

fn saying(hello: Hello) -> Vec<u8> {
    encode(&Packet {
        control: Some(Control::Hello(hello)),
        ..Packet::default()
    })
}

/// A packet from a welcomed device.
fn from_device() -> Packet {
    Packet {
        epoch: EPOCH,
        ..Packet::default()
    }
}

struct Outputs {
    packets: Vec<(LinkId, Packet)>,
    closed: Vec<LinkId>,
    events: Vec<DeviceEvent<u32>>,
}

fn drain(room: &mut Room, now: Ms) -> Outputs {
    let mut outputs = Outputs {
        packets: Vec::new(),
        closed: Vec::new(),
        events: Vec::new(),
    };
    while let Some(output) = room.poll(now) {
        match output {
            Output::Transmit { link, bytes } => outputs.packets.push((link, decode(&bytes))),
            Output::Close { link } => outputs.closed.push(link),
            Output::Device(event) => outputs.events.push(event),
        }
    }
    outputs
}

/// A phone on `link`, welcomed at `now`.
fn join(room: &mut Room, now: Ms, link: LinkId, token: u8) -> DeviceId {
    room.opened(now, link, Caps::DATAGRAM);
    room.received(now, link, &saying(hello(token)));
    let outputs = drain(room, now);
    match outputs.events.as_slice() {
        [DeviceEvent::Arrived { device, .. }] => *device,
        other => panic!("{other:?}"),
    }
}

fn joined() -> (Room, DeviceId) {
    let mut room = room();
    let device = join(&mut room, Ms(0), LINK, 1);
    (room, device)
}

#[test]
fn a_packet_of_exactly_64_kib_is_read_and_one_byte_more_closes_the_link() {
    let mut host = room();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    host.received(Ms(0), LINK, &vec![0xff; 64 * 1024]);
    assert_eq!(drain(&mut host, Ms(0)).closed, Vec::new());
    host.received(Ms(0), LINK, &vec![0xff; 64 * 1024 + 1]);
    assert_eq!(drain(&mut host, Ms(0)).closed, vec![LINK]);
}

/// A stranger gets three chances, a known device as many as it wants: its
/// garbage is counted, and it stays.
#[test]
fn garbage_drops_a_stranger_at_the_third_packet_and_never_a_device() {
    let mut host = room();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    for _ in 0..2 {
        host.received(Ms(0), LINK, &[0xff]);
        assert_eq!(drain(&mut host, Ms(0)).closed, Vec::new());
    }
    host.received(Ms(0), LINK, &[0xff]);
    assert_eq!(drain(&mut host, Ms(0)).closed, vec![LINK]);

    let (mut host, device) = joined();
    for count in 1..=10 {
        host.received(Ms(0), LINK, &[0xff]);
        assert_eq!(drain(&mut host, Ms(0)).closed, Vec::new());
        assert_eq!(host.device(device).unwrap().garbage, count);
    }
}

/// Whatever brings data is acknowledged at once, event or state; a packet
/// without any waits for the heartbeat.
#[test]
fn data_is_acknowledged_at_once_and_only_data() {
    let bytes = postcard::to_allocvec(&5_u32).unwrap();
    let cases = [
        (vec![event(0, bytes.clone())], vec![], 1),
        (vec![], vec![state(1, bytes)], 1),
        (vec![], vec![], 0),
    ];
    for (events, states, replies) in cases {
        let (mut host, _) = joined();
        let packet = Packet {
            events,
            state: states,
            ..from_device()
        };
        host.received(Ms(10), LINK, &encode(&packet));
        assert_eq!(drain(&mut host, Ms(10)).packets.len(), replies);
    }
}

#[test]
fn holding_nothing_is_not_holding_state_zero() {
    let (mut host, device) = joined();
    let holding = |seq| {
        encode(&Packet {
            holding: Some(seq),
            ..from_device()
        })
    };
    host.received(Ms(10), LINK, &holding(0));
    assert_eq!(host.device(device).unwrap().held_at, None);

    let bytes = postcard::to_allocvec(&5_u32).unwrap();
    let packet = Packet {
        state: vec![state(2, bytes)],
        ..from_device()
    };
    host.received(Ms(20), LINK, &encode(&packet));
    assert_eq!(host.device(device).unwrap().held_at, Some(Ms(20)));
    // An older number is not the state the game knows.
    host.received(Ms(30), LINK, &holding(1));
    assert_eq!(host.device(device).unwrap().held_at, Some(Ms(20)));
    host.received(Ms(40), LINK, &holding(2));
    assert_eq!(host.device(device).unwrap().held_at, Some(Ms(40)));
}

fn tv_hello() -> Hello {
    Hello {
        kind: Kind::Tv,
        key: Some(KEY),
        ..hello(1)
    }
}

/// A phone in a browser may become the app; the TV stays the TV, and comes
/// back as itself.
#[test]
fn a_device_comes_back_under_a_kind_it_may_have() {
    let (mut host, device) = joined();
    host.closed(Ms(1), LINK);
    drain(&mut host, Ms(1));
    let other = LinkId(2);
    host.opened(Ms(2), other, Caps::DATAGRAM);
    host.received(
        Ms(2),
        other,
        &saying(Hello {
            kind: Kind::App,
            ..hello(1)
        }),
    );
    let outputs = drain(&mut host, Ms(2));
    assert_eq!(outputs.events, vec![DeviceEvent::Back { device }]);
    assert_eq!(host.device(device).unwrap().kind, Kind::App);

    let mut host = room();
    let tv = Caps::DATAGRAM.local();
    host.opened(Ms(0), LINK, tv);
    host.received(Ms(0), LINK, &saying(tv_hello()));
    let outputs = drain(&mut host, Ms(0));
    let [DeviceEvent::Arrived { device, .. }] = outputs.events.as_slice() else {
        panic!("{:?}", outputs.events);
    };
    let device = *device;
    host.closed(Ms(1), LINK);
    drain(&mut host, Ms(1));
    host.opened(Ms(2), LinkId(2), tv);
    host.received(Ms(2), LinkId(2), &saying(tv_hello()));
    let outputs = drain(&mut host, Ms(2));
    assert_eq!(outputs.events, vec![DeviceEvent::Back { device }]);
}

/// A refusal goes three times and the link stays open a second for them, so
/// one lost packet leaves no phone guessing.
#[test]
fn a_refused_link_closes_after_its_grace_and_not_before() {
    let mut host = room();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    host.received(
        Ms(0),
        LINK,
        &saying(Hello {
            game: GAME + 1,
            ..hello(1)
        }),
    );
    let outputs = drain(&mut host, Ms(0));
    let refusals = outputs
        .packets
        .iter()
        .filter(|(_, packet)| packet.control == Some(Control::Refused(Refusal::Version)))
        .count();
    assert_eq!(refusals, 3);
    assert_eq!(outputs.closed, Vec::new());
    assert_eq!(host.next_deadline(), Some(Ms(1_000)));
    assert_eq!(drain(&mut host, Ms(999)).closed, Vec::new());
    assert_eq!(drain(&mut host, Ms(1_000)).closed, vec![LINK]);
    assert_eq!(host.next_deadline(), None);
}

fn quiet_room() -> Room {
    Room::new(
        Ms(0),
        HostConfig {
            timing: Timing {
                heartbeat: 100_000,
                ..Timing::default()
            },
            ..config(Limits::default(), Vec::new())
        },
    )
}

#[test]
fn the_next_deadline_is_none_with_nothing_to_watch() {
    assert_eq!(room().next_deadline(), None);
}

#[test]
fn the_next_deadline_is_now_while_something_waits() {
    // A device event for the game.
    let mut host = room();
    host.add_local(Ms(5), Kind::Gamepad, None);
    assert_eq!(host.next_deadline(), Some(Ms(5)));

    // An output for the shell.
    let mut host = room();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    host.received(Ms(5), LINK, &vec![0; 64 * 1024 + 1]);
    assert_eq!(host.next_deadline(), Some(Ms(5)));

    // A control for a link.
    let mut host = room();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    host.received(
        Ms(5),
        LINK,
        &saying(Hello {
            game: GAME + 1,
            ..hello(1)
        }),
    );
    assert_eq!(host.next_deadline(), Some(Ms(5)));

    // An acknowledgement, for a repeat that delivers nothing new.
    let (mut host, _) = joined();
    let repeat = Packet {
        events: vec![event(9, Vec::new())],
        ..from_device()
    };
    host.received(Ms(5), LINK, &encode(&repeat));
    assert_eq!(host.next_deadline(), Some(Ms(5)));
}

#[test]
fn the_next_deadline_watches_each_link() {
    // A stranger, until its handshake runs out.
    let mut host = quiet_room();
    host.opened(Ms(10), LINK, Caps::DATAGRAM);
    assert_eq!(host.next_deadline(), Some(Ms(10_010)));

    // A present device goes weak first, a weak one goes absent.
    let mut host = quiet_room();
    let device = join(&mut host, Ms(0), LINK, 1);
    assert_eq!(host.next_deadline(), Some(Ms(1_500)));
    drain(&mut host, Ms(1_500));
    assert_eq!(host.device(device).unwrap().presence, Presence::Weak);
    assert_eq!(host.next_deadline(), Some(Ms(6_000)));

    // What waits goes again when due, and the first time at once.
    let mut host = quiet_room();
    let device = join(&mut host, Ms(0), LINK, 1);
    host.send(device, &vec![1]).unwrap();
    assert_eq!(host.next_deadline(), Some(Ms(0)));
    drain(&mut host, Ms(10));
    assert_eq!(host.next_deadline(), Some(Ms(260)));

    let mut host = quiet_room();
    let device = join(&mut host, Ms(0), LINK, 1);
    host.show(device, &vec![1]).unwrap();
    assert_eq!(host.next_deadline(), Some(Ms(0)));
    drain(&mut host, Ms(10));
    assert_eq!(host.next_deadline(), Some(Ms(260)));
}

#[test]
fn a_message_of_exactly_the_limit_goes_and_a_longer_one_does_not() {
    let limits = Limits {
        max_down: 16,
        ..Limits::default()
    };
    let mut host = Room::new(Ms(0), config(limits, Vec::new()));
    let device = join(&mut host, Ms(0), LINK, 1);
    // One byte of length, then the bytes.
    assert_eq!(host.send(device, &vec![1; 15]), Ok(()));
    assert_eq!(host.send(device, &vec![1; 16]), Err(SendError::TooBig));
    assert_eq!(host.send(device, &vec![1; 17]), Err(SendError::TooBig));
    assert_eq!(host.show(device, &vec![1; 15]), Ok(()));
    assert_eq!(host.show(device, &vec![1; 17]), Err(SendError::TooBig));
}

#[test]
fn the_phase_is_what_the_game_set() {
    let mut host = room();
    assert_eq!(host.phase(), 0);
    host.set_phase(5);
    assert_eq!(host.phase(), 5);
}

#[test]
fn a_closed_room_opens_again() {
    let mut host = room();
    host.close();
    host.opened(Ms(0), LINK, Caps::DATAGRAM);
    host.received(Ms(0), LINK, &saying(hello(1)));
    assert_eq!(drain(&mut host, Ms(0)).events, Vec::new());
    host.open();
    host.opened(Ms(1), LinkId(2), Caps::DATAGRAM);
    host.received(Ms(1), LinkId(2), &saying(hello(1)));
    assert_eq!(drain(&mut host, Ms(1)).events.len(), 1);
}

/// Forgotten: told so, out of the room, its token is nobody's.
#[test]
fn a_forgotten_device_is_told_and_gone() {
    let (mut host, device) = joined();
    host.forget(device);
    let outputs = drain(&mut host, Ms(1));
    assert!(
        outputs
            .packets
            .iter()
            .any(|(_, packet)| packet.control == Some(Control::Refused(Refusal::Removed)))
    );
    assert_eq!(host.device(device), None);
    assert_eq!(host.known(), Vec::new());
    assert_eq!(host.send(device, &vec![1]), Err(SendError::Unknown));
    // Forgetting nobody is nothing.
    host.forget(DeviceId(99));
}

#[test]
fn a_gamepad_can_be_unplugged_once_and_a_phone_never() {
    let (mut host, phone) = joined();
    host.local_unplugged(Ms(1), phone);
    assert_eq!(drain(&mut host, Ms(1)).events, Vec::new());
    assert_eq!(
        host.device(phone).unwrap().presence,
        Presence::Present,
        "a phone is not a cable"
    );
    host.local_plugged(Ms(2), phone);
    assert_eq!(drain(&mut host, Ms(2)).events, Vec::new());

    let mut host = room();
    let pad = host.add_local(Ms(0), Kind::Gamepad, None);
    drain(&mut host, Ms(0));
    host.local_plugged(Ms(1), pad);
    assert_eq!(drain(&mut host, Ms(1)).events, Vec::new(), "already in");
    host.local_unplugged(Ms(2), pad);
    let left = DeviceEvent::Left {
        device: pad,
        why: Why::Closed,
    };
    assert_eq!(drain(&mut host, Ms(2)).events, vec![left]);
    host.local_unplugged(Ms(3), pad);
    assert_eq!(drain(&mut host, Ms(3)).events, Vec::new(), "already out");
}

#[test]
fn only_a_local_device_sends_local_events() {
    let mut host = room();
    let pad = host.add_local(Ms(0), Kind::Keyboard, None);
    drain(&mut host, Ms(0));
    host.set_phase(4);
    host.local_event(Ms(1), pad, 8);
    let want = DeviceEvent::Event {
        device: pad,
        event: 8,
        phase: 4,
        at: Ms(1),
    };
    assert_eq!(drain(&mut host, Ms(1)).events, vec![want]);

    let (mut host, phone) = joined();
    host.local_event(Ms(1), phone, 8);
    assert_eq!(drain(&mut host, Ms(1)).events, Vec::new());
}

/// The numbers a welcome gives: where the device's events go on, and the
/// oldest event the host still has for it.
#[test]
fn a_welcome_tells_where_both_numberings_are() {
    let (mut host, device) = joined();
    host.send(device, &vec![1]).unwrap();
    host.send(device, &vec![2]).unwrap();
    drain(&mut host, Ms(1));
    let packet = Packet {
        events: vec![event(0, postcard::to_allocvec(&5_u32).unwrap())],
        ack: Some(Ack {
            events: 1,
            state: 0,
        }),
        ..from_device()
    };
    host.received(Ms(2), LINK, &encode(&packet));
    drain(&mut host, Ms(2));

    host.closed(Ms(3), LINK);
    drain(&mut host, Ms(3));
    let other = LinkId(2);
    host.opened(Ms(4), other, Caps::DATAGRAM);
    host.received(Ms(4), other, &saying(hello(1)));
    let outputs = drain(&mut host, Ms(4));
    let welcome = outputs
        .packets
        .iter()
        .find_map(|(_, packet)| match &packet.control {
            Some(Control::Welcome(welcome)) => Some(*welcome),
            _ => None,
        })
        .expect("a welcome");
    assert_eq!((welcome.up_next, welcome.down_first), (1, 1));
    // And what was not acknowledged goes at once on the new link.
    let again: Vec<u32> = outputs
        .packets
        .iter()
        .flat_map(|(_, packet)| &packet.events)
        .map(|part| part.seq)
        .collect();
    assert_eq!(again, vec![1]);
}

/// With nothing queued, the oldest is the next number.
#[test]
fn a_welcome_with_nothing_queued_names_the_next_number() {
    let (mut host, device) = joined();
    host.send(device, &vec![1]).unwrap();
    drain(&mut host, Ms(1));
    let ack = Packet {
        ack: Some(Ack {
            events: 1,
            state: 0,
        }),
        ..from_device()
    };
    host.received(Ms(2), LINK, &encode(&ack));
    host.closed(Ms(3), LINK);
    drain(&mut host, Ms(3));
    host.opened(Ms(4), LinkId(2), Caps::DATAGRAM);
    host.received(Ms(4), LinkId(2), &saying(hello(1)));
    let welcome = drain(&mut host, Ms(4))
        .packets
        .iter()
        .find_map(|(_, packet)| match &packet.control {
            Some(Control::Welcome(welcome)) => Some(*welcome),
            _ => None,
        })
        .expect("a welcome");
    assert_eq!(welcome.down_first, 1);
}

/// A new link sends the latest state again at once, if there is one, even
/// acknowledged.
#[test]
fn a_new_link_gets_the_latest_state_if_there_is_one() {
    let (mut host, device) = joined();
    host.closed(Ms(1), LINK);
    drain(&mut host, Ms(1));
    host.opened(Ms(2), LinkId(2), Caps::DATAGRAM);
    host.received(Ms(2), LinkId(2), &saying(hello(1)));
    let states = drain(&mut host, Ms(2))
        .packets
        .iter()
        .map(|(_, packet)| packet.state.len())
        .sum::<usize>();
    assert_eq!(states, 0, "no state yet");

    host.show(device, &vec![1]).unwrap();
    drain(&mut host, Ms(3));
    let ack = Packet {
        ack: Some(Ack {
            events: 0,
            state: 1,
        }),
        ..from_device()
    };
    host.received(Ms(4), LinkId(2), &encode(&ack));
    host.closed(Ms(5), LinkId(2));
    drain(&mut host, Ms(5));
    host.opened(Ms(6), LinkId(3), Caps::DATAGRAM);
    host.received(Ms(6), LinkId(3), &saying(hello(1)));
    let states = drain(&mut host, Ms(6))
        .packets
        .iter()
        .map(|(_, packet)| packet.state.len())
        .sum::<usize>();
    assert_eq!(states, 1);
}

#[test]
fn a_known_device_keeps_what_the_game_named() {
    let known = Known {
        device: DeviceId(4),
        token: Token([1; 16]),
        kind: Kind::Phone,
        name: Name::new("Ada"),
    };
    let host = Room::new(Ms(0), config(Limits::default(), vec![known.clone()]));
    assert_eq!(host.known(), vec![known]);
}

#[test]
fn the_echo_and_the_held_time_go_back_to_the_device() {
    let (mut host, device) = joined();
    let packet = Packet {
        sent: 900,
        ..from_device()
    };
    host.received(Ms(1_000), LINK, &encode(&packet));
    drain(&mut host, Ms(1_000));
    host.send(device, &vec![1]).unwrap();
    let outputs = drain(&mut host, Ms(1_040));
    assert_eq!(
        outputs.packets[0].1.echo,
        Some(Echo {
            sent: 900,
            held: 40
        })
    );
}

#[test]
fn a_new_link_measures_from_zero() {
    let (mut host, device) = joined();
    let echoed = Packet {
        sent: 1_100,
        echo: Some(Echo {
            sent: 1_000,
            held: 0,
        }),
        ..from_device()
    };
    host.received(Ms(1_100), LINK, &encode(&echoed));
    assert_eq!(host.device(device).unwrap().rtt, Some(100));
    host.closed(Ms(1_200), LINK);
    drain(&mut host, Ms(1_200));
    host.opened(Ms(1_300), LinkId(2), Caps::DATAGRAM);
    host.received(Ms(1_300), LinkId(2), &saying(hello(1)));
    assert_eq!(host.device(device).unwrap().rtt, None);
}
