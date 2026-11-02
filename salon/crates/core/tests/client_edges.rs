//! The device's side, edge by edge, each found by a mutant that survived the
//! simulated network: the shell's calls go in, and the host is played by hand
//! from forged packets, so every wait and every output can be counted.

#![expect(
    clippy::indexing_slicing,
    clippy::expect_used,
    reason = "test helpers outside #[test] functions: a missing packet stops the test"
)]

mod wire;

use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, Kind, Limits, Ms, Status, Timing, Token,
};
use wire::{Ack, Control, Packet, decode, encode, event, from_host, state, welcome};

type Phone = Client<u32, u32>;
const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

fn config(seed: u64, timing: Timing) -> ClientConfig {
    ClientConfig {
        game: 7,
        kind: Kind::Phone,
        token: Token([1; 16]),
        name: None,
        tv_key: None,
        seed,
        timing,
        limits: Limits::default(),
    }
}

fn drain(phone: &mut Phone, now: Ms) -> Vec<ClientOutput<u32>> {
    std::iter::from_fn(|| phone.poll(now)).collect()
}

fn sent(outputs: &[ClientOutput<u32>]) -> Vec<Packet> {
    outputs
        .iter()
        .filter_map(|output| {
            if let ClientOutput::Transmit(bytes) = output {
                Some(decode(bytes))
            } else {
                None
            }
        })
        .collect()
}

fn has(outputs: &[ClientOutput<u32>], wanted: &ClientOutput<u32>) -> usize {
    outputs.iter().filter(|output| *output == wanted).count()
}

/// When the next attempt is due, once the shell heard everything.
fn next_attempt(phone: &mut Phone, now: Ms) -> Option<Ms> {
    drain(phone, now);
    phone.next_deadline()
}

/// Fails `failures` attempts in a row, and returns when the next one is due.
fn fail_attempts(phone: &mut Phone, mut now: Ms, failures: usize) -> Ms {
    for _ in 0..failures {
        let outputs = drain(phone, now);
        assert_eq!(has(&outputs, &ClientOutput::Connect), 1, "at {now:?}");
        phone.closed(now);
        now = next_attempt(phone, now).expect("a planned attempt");
    }
    now
}

/// Opens a transport at `now` and has it welcomed.
fn connect(phone: &mut Phone, now: Ms, epoch: u32) {
    let outputs = drain(phone, now);
    assert_eq!(has(&outputs, &ClientOutput::Connect), 1, "at {now:?}");
    phone.opened(now, Caps::DATAGRAM);
    let hello = sent(&drain(phone, now));
    assert!(matches!(hello[0].control, Some(Control::Hello(_))));
    phone.received(now, &welcome(4, epoch, 0, 0));
    drain(phone, now);
    assert_eq!(phone.status(), Status::Connected);
}

fn joined(now: Ms, timing: Timing) -> Phone {
    let mut phone = Phone::new(now, config(1, timing));
    connect(&mut phone, now, 1);
    phone
}

/// SplitMix64 from seed 1234567: the published test vector, which fixes both
/// the mixing and the ±25 % around each doubling wait.
#[test]
fn the_waits_double_with_a_quarter_of_chance_each_way() {
    let draws: [u64; 5] = [
        6_457_827_717_110_365_317,
        3_203_168_211_198_807_973,
        9_817_491_932_198_370_423,
        4_593_380_528_125_082_431,
        16_408_922_859_458_223_821,
    ];
    let start = Ms(1_000);
    let mut phone = Phone::new(start, config(0x0012_d687 ^ GOLDEN, Timing::default()));
    let mut now = start;
    for (attempt, draw) in draws.into_iter().enumerate() {
        let base = (250_u64 << attempt).min(2_000);
        let want = base - base / 4 + draw % (base / 2);
        let due = fail_attempts(&mut phone, now, 1);
        assert_eq!(due.since(now), want, "attempt {attempt}");
        // Not one millisecond early.
        assert_eq!(drain(&mut phone, Ms(due.0 - 1)), Vec::new());
        now = due;
    }
}

#[test]
fn two_phones_spread_their_attempts() {
    let wait = |seed| {
        let mut phone = Phone::new(Ms(0), config(seed, Timing::default()));
        fail_attempts(&mut phone, Ms(0), 3).0
    };
    let waits: std::collections::BTreeSet<u64> = (0..64).map(wait).collect();
    assert!(waits.len() > 32, "only {} waits", waits.len());
}

/// After four failures, the next wait after a transport's end is the first
/// one if it lived `healthy`, and the long one if not.
#[test]
fn only_a_link_that_lived_long_enough_resets_the_waits() {
    let timing = Timing::default();
    let first_wait = (timing.retry_first * 3 / 4)..(timing.retry_first * 5 / 4);
    let long_wait = (timing.retry_max * 3 / 4)..(timing.retry_max * 5 / 4);

    let mut phone = Phone::new(Ms(0), config(1, timing));
    let now = fail_attempts(&mut phone, Ms(0), 4);
    connect(&mut phone, now, 1);
    let later = now.plus(timing.healthy);
    phone.closed(later);
    let wait = next_attempt(&mut phone, later).unwrap().since(later);
    assert!(first_wait.contains(&wait), "healthy link, then {wait}");

    let mut phone = Phone::new(Ms(0), config(1, timing));
    let now = fail_attempts(&mut phone, Ms(0), 4);
    connect(&mut phone, now, 1);
    let later = now.plus(timing.healthy - 1);
    phone.closed(later);
    let wait = next_attempt(&mut phone, later).unwrap().since(later);
    assert!(long_wait.contains(&wait), "short link, then {wait}");

    // A link nobody welcomed proves nothing, however long it stayed open.
    let mut phone = Phone::new(Ms(0), config(1, timing));
    let now = fail_attempts(&mut phone, Ms(0), 4);
    drain(&mut phone, now);
    phone.opened(now, Caps::DATAGRAM);
    let later = now.plus(timing.handshake);
    phone.tick(later);
    let wait = next_attempt(&mut phone, later).unwrap().since(later);
    assert!(long_wait.contains(&wait), "never welcomed, then {wait}");
}

/// The watchdog retries at once only if no attempt failed since a healthy
/// link: otherwise a dying host would be hammered.
#[test]
fn the_watchdog_retries_at_once_only_after_a_healthy_link() {
    let timing = Timing::default();
    let mut phone = Phone::new(Ms(0), config(1, timing));
    let now = fail_attempts(&mut phone, Ms(0), 4);
    connect(&mut phone, now, 1);
    let dead = now.plus(timing.absent);
    phone.tick(dead);
    assert_eq!(phone.next_deadline(), Some(dead), "healthy: at once");
    assert_eq!(has(&drain(&mut phone, dead), &ClientOutput::Connect), 1);

    let timing = Timing {
        healthy: 60_000,
        ..Timing::default()
    };
    let mut phone = Phone::new(Ms(0), config(1, timing));
    let now = fail_attempts(&mut phone, Ms(0), 4);
    connect(&mut phone, now, 1);
    let dead = now.plus(timing.absent);
    phone.tick(dead);
    let wait = next_attempt(&mut phone, dead).unwrap().since(dead);
    assert!(wait >= 1_500, "not healthy: after the backoff, not {wait}");
}

/// The host says it stops: never at once, even after a healthy link.
#[test]
fn a_closing_host_is_retried_after_a_wait() {
    let timing = Timing::default();
    let mut phone = joined(Ms(0), timing);
    let later = Ms(timing.healthy);
    let closing = Packet {
        control: Some(Control::Closing),
        ..from_host(1, later.0)
    };
    phone.received(later, &encode(&closing));
    assert_eq!(phone.status(), Status::HostGone);
    let outputs = drain(&mut phone, later);
    assert_eq!(has(&outputs, &ClientOutput::Connect), 0, "not at once");
    let wait = phone.next_deadline().unwrap().since(later);
    assert!(wait >= timing.retry_first * 3 / 4, "after {wait}");
}

#[test]
fn stopping_drops_an_open_transport_once() {
    let mut phone = joined(Ms(0), Timing::default());
    let refused = Packet {
        control: Some(Control::Refused(salon_core::Refusal::Removed)),
        ..from_host(1, 10)
    };
    phone.received(Ms(10), &encode(&refused));
    let outputs = drain(&mut phone, Ms(10));
    assert_eq!(has(&outputs, &ClientOutput::Disconnect), 1);
    assert_eq!(next_attempt(&mut phone, Ms(10)), None);

    // Nothing is open: nothing to drop.
    let mut idle = Phone::new(Ms(0), config(1, Timing::default()));
    idle.quit();
    assert_eq!(idle.status(), Status::Quit);
    assert_eq!(has(&drain(&mut idle, Ms(0)), &ClientOutput::Disconnect), 0);
}

#[test]
fn a_transport_closing_while_goodbyes_wait_ends_the_visit() {
    let mut phone = joined(Ms(0), Timing::default());
    phone.quit();
    phone.closed(Ms(1));
    assert_eq!(phone.status(), Status::Quit);
    assert_eq!(
        next_attempt(&mut phone, Ms(1)),
        None,
        "no reconnection after quitting"
    );
}

#[test]
fn a_goodbye_goes_three_times_and_alone_at_the_end() {
    let mut phone = joined(Ms(0), Timing::default());
    phone.send(&1).unwrap();
    phone.quit();
    let outputs = drain(&mut phone, Ms(1));
    let packets = sent(&outputs);
    let byes = packets
        .iter()
        .filter(|packet| packet.control == Some(Control::Bye))
        .count();
    assert_eq!(byes, 3, "{packets:?}");
    let last = packets.last().unwrap();
    assert_eq!(last.control, Some(Control::Bye));
    assert_eq!(last.ack, None, "the last one carries nothing else");
    assert_eq!(has(&outputs, &ClientOutput::Disconnect), 1);
}

/// Waking up with a live link must not cut it: a phone would drop its
/// connection each time its page gets the focus back.
#[test]
fn waking_up_leaves_a_live_link_alone() {
    let timing = Timing::default();
    let mut phone = joined(Ms(0), timing);
    let just_before = Ms(timing.silent - 1);
    phone.wake(just_before);
    assert_eq!(
        has(&drain(&mut phone, just_before), &ClientOutput::Disconnect),
        0
    );
    assert_eq!(phone.status(), Status::Connected);
    phone.wake(Ms(timing.silent));
    assert_eq!(
        has(
            &drain(&mut phone, Ms(timing.silent)),
            &ClientOutput::Disconnect
        ),
        1
    );
}

#[test]
fn a_packet_of_another_run_is_ignored() {
    let mut phone = joined(Ms(0), Timing::default());
    let other = Packet {
        events: vec![event(0, postcard::to_allocvec(&5_u32).unwrap())],
        ack: Some(Ack {
            events: 0,
            state: 0,
            ..Ack::default()
        }),
        ..from_host(2, 5)
    };
    phone.received(Ms(5), &encode(&other));
    let outputs = drain(&mut phone, Ms(5));
    assert_eq!(has(&outputs, &ClientOutput::Event(5)), 0);
}

/// A new link and no welcome yet: the old run's numbering means nothing until
/// the host answers.
#[test]
fn nothing_counts_before_the_welcome_of_a_new_link() {
    let mut phone = joined(Ms(0), Timing::default());
    phone.closed(Ms(10));
    let due = next_attempt(&mut phone, Ms(10)).unwrap();
    assert_eq!(has(&drain(&mut phone, due), &ClientOutput::Connect), 1);
    phone.opened(due, Caps::DATAGRAM);
    drain(&mut phone, due);
    let early = Packet {
        events: vec![event(0, postcard::to_allocvec(&5_u32).unwrap())],
        ..from_host(1, 5)
    };
    phone.received(due, &encode(&early));
    assert_eq!(has(&drain(&mut phone, due), &ClientOutput::Event(5)), 0);
}

/// Whatever brings data is acknowledged at once, event or state; what
/// brings none waits for the heartbeat.
#[test]
fn data_is_acknowledged_at_once_and_only_data() {
    let bytes = postcard::to_allocvec(&5_u32).unwrap();
    let cases = [
        (vec![event(0, bytes.clone())], vec![], true),
        (vec![], vec![state(1, bytes)], true),
        (vec![], vec![], false),
    ];
    for (events, states, due) in cases {
        let mut phone = joined(Ms(0), Timing::default());
        let now = Ms(10);
        drain(&mut phone, now);
        let packet = Packet {
            events,
            state: states,
            ..from_host(1, 10)
        };
        phone.received(now, &encode(&packet));
        let outputs = drain(&mut phone, now);
        assert_eq!(sent(&outputs).len(), usize::from(due), "{outputs:?}");
    }
}

#[test]
fn a_hello_goes_again_after_the_resend_delay_and_not_before() {
    let mut phone = Phone::new(Ms(0), config(1, Timing::default()));
    drain(&mut phone, Ms(0));
    phone.opened(Ms(0), Caps::DATAGRAM);
    assert_eq!(sent(&drain(&mut phone, Ms(0))).len(), 1);
    assert_eq!(sent(&drain(&mut phone, Ms(249))).len(), 0);
    assert_eq!(sent(&drain(&mut phone, Ms(250))).len(), 1);
}

/// A hello is a question: what the player did waits for the answer, with its
/// number from the welcome.
#[test]
fn a_hello_carries_nothing_else() {
    let mut phone = Phone::new(Ms(0), config(1, Timing::default()));
    phone.send(&1).unwrap();
    phone.set_state(&2).unwrap();
    drain(&mut phone, Ms(0));
    phone.opened(Ms(0), Caps::DATAGRAM);
    let packets = sent(&drain(&mut phone, Ms(0)));
    assert_eq!(packets.len(), 1);
    assert_eq!((packets[0].events.len(), packets[0].state.len()), (0, 0));
}

#[test]
fn a_phone_with_no_state_holds_nothing() {
    let mut phone = joined(Ms(0), Timing::default());
    let packets = sent(&drain(&mut phone, Ms(250)));
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].holding, None);
    phone.set_state(&1).unwrap();
    let packets = sent(&drain(&mut phone, Ms(251)));
    assert_eq!(packets[0].holding, Some(1));
}

#[test]
fn a_big_message_leaves_in_one_flush() {
    let mut phone: Client<Vec<u8>, u32> = Client::new(Ms(0), config(1, Timing::default()));
    std::iter::from_fn(|| phone.poll(Ms(0))).for_each(drop);
    phone.opened(Ms(0), Caps::DATAGRAM);
    std::iter::from_fn(|| phone.poll(Ms(0))).for_each(drop);
    phone.received(Ms(0), &welcome(4, 1, 0, 0));
    std::iter::from_fn(|| phone.poll(Ms(0))).for_each(drop);
    // Three fragments of a kilobyte, so three packets at the same instant.
    phone.send(&vec![7; 2_500]).unwrap();
    let packets = std::iter::from_fn(|| phone.poll(Ms(1)))
        .filter(|output| matches!(output, ClientOutput::Transmit(_)))
        .count();
    assert_eq!(packets, 3);
}

#[test]
fn the_next_deadline_is_now_while_something_waits() {
    let timing = Timing::default();
    // Nothing waits: the next heartbeat.
    let mut phone = joined(Ms(0), timing);
    assert_eq!(phone.next_deadline(), Some(Ms(timing.heartbeat)));

    // A goodbye, once the shell read the status.
    phone.quit();
    assert_eq!(
        phone.poll(Ms(1)),
        Some(ClientOutput::Status(Status::Quit)),
        "the status comes first"
    );
    assert_eq!(phone.next_deadline(), Some(Ms(1)));

    // An acknowledgement, even for a message that does not read.
    let mut phone = joined(Ms(0), timing);
    let packet = Packet {
        events: vec![event(0, vec![0xff; 8])],
        ..from_host(1, 2)
    };
    phone.received(Ms(2), &encode(&packet));
    assert_eq!(phone.next_deadline(), Some(Ms(2)));
}

#[test]
fn a_message_of_exactly_the_limit_goes_and_a_longer_one_does_not() {
    use salon_core::ClientSendError::TooBig;
    let limits = Limits {
        max_up: 16,
        ..Limits::default()
    };
    let mut phone: Client<Vec<u8>, u32> = Client::new(
        Ms(0),
        ClientConfig {
            limits,
            ..config(1, Timing::default())
        },
    );
    // One byte of length, then the bytes.
    assert_eq!(phone.send(&vec![1; 15]), Ok(()));
    assert_eq!(phone.send(&vec![1; 16]), Err(TooBig));
    assert_eq!(phone.send(&vec![1; 17]), Err(TooBig));
    assert_eq!(phone.set_state(&vec![1; 15]), Ok(()));
    assert_eq!(phone.set_state(&vec![1; 17]), Err(TooBig));
}

#[test]
fn quitting_before_the_welcome_just_closes() {
    let mut phone = Phone::new(Ms(0), config(1, Timing::default()));
    drain(&mut phone, Ms(0));
    phone.opened(Ms(0), Caps::DATAGRAM);
    drain(&mut phone, Ms(0));
    phone.quit();
    let outputs = drain(&mut phone, Ms(1));
    assert_eq!(sent(&outputs), Vec::new(), "nobody to say goodbye to");
    assert_eq!(has(&outputs, &ClientOutput::Disconnect), 1);
}

#[test]
fn the_round_trip_is_measured_and_smoothed() {
    let mut phone = joined(Ms(0), Timing::default());
    assert_eq!(phone.rtt(), None);
    let echoed = |at: u64, sent: u64, held: u32| {
        encode(&Packet {
            echo: Some(wire::Echo { sent, held }),
            ..from_host(1, at)
        })
    };
    // Sent at 1 000, held 40 ms by the host, back at 1 100.
    phone.received(Ms(1_100), &echoed(1_080, 1_000, 40));
    assert_eq!(phone.rtt(), Some(60));
    // A tenth of the way towards 160.
    phone.received(Ms(2_160), &echoed(2_100, 2_000, 0));
    assert_eq!(phone.rtt(), Some(70));
}

/// Waits long enough that only `unsend` can explain what goes at once.
fn patient() -> Timing {
    Timing {
        resend_min: 60_000,
        resend_max: 60_000,
        ..Timing::default()
    }
}

/// The transport dies and the next one is open, not yet welcomed.
fn reopen(phone: &mut Phone, now: Ms) -> Ms {
    phone.closed(now);
    let due = next_attempt(phone, now).expect("a planned attempt");
    assert_eq!(has(&drain(phone, due), &ClientOutput::Connect), 1);
    phone.opened(due, Caps::DATAGRAM);
    drain(phone, due);
    due
}

fn event_seqs(packets: &[Packet]) -> Vec<u32> {
    packets
        .iter()
        .flat_map(|packet| &packet.events)
        .map(|part| part.seq)
        .collect()
}

fn state_seqs(packets: &[Packet]) -> Vec<u32> {
    packets
        .iter()
        .flat_map(|packet| &packet.state)
        .map(|part| part.seq)
        .collect()
}

/// What the player did while connecting counts, numbered from the host's
/// first number for it.
#[test]
fn the_first_welcome_numbers_what_waited() {
    let mut phone = Phone::new(Ms(0), config(1, Timing::default()));
    phone.send(&1).unwrap();
    phone.send(&2).unwrap();
    drain(&mut phone, Ms(0));
    phone.opened(Ms(0), Caps::DATAGRAM);
    drain(&mut phone, Ms(0));
    phone.received(Ms(1), &welcome(4, 1, 5, 0));
    assert_eq!(event_seqs(&sent(&drain(&mut phone, Ms(1)))), vec![5, 6]);
    phone.send(&3).unwrap();
    assert_eq!(event_seqs(&sent(&drain(&mut phone, Ms(2)))), vec![7]);
}

#[test]
fn the_first_welcome_says_where_the_hosts_events_start() {
    let mut phone = Phone::new(Ms(0), config(1, Timing::default()));
    drain(&mut phone, Ms(0));
    phone.opened(Ms(0), Caps::DATAGRAM);
    drain(&mut phone, Ms(0));
    phone.received(Ms(1), &welcome(4, 1, 0, 7));
    drain(&mut phone, Ms(1));
    let packet = Packet {
        events: vec![event(7, postcard::to_allocvec(&5_u32).unwrap())],
        ..from_host(1, 2)
    };
    phone.received(Ms(2), &encode(&packet));
    assert_eq!(has(&drain(&mut phone, Ms(2)), &ClientOutput::Event(5)), 1);
}

/// Back on the same run: what the host has is gone from the queue, and the
/// rest goes at once on the new link, without waiting for a resend.
#[test]
fn a_new_link_sends_at_once_what_the_host_does_not_have() {
    let mut phone = joined(Ms(0), patient());
    phone.send(&1).unwrap();
    phone.send(&2).unwrap();
    phone.set_state(&3).unwrap();
    assert_eq!(event_seqs(&sent(&drain(&mut phone, Ms(1)))), vec![0, 1]);
    let now = reopen(&mut phone, Ms(2));
    phone.received(now, &welcome(4, 1, 1, 0));
    let packets = sent(&drain(&mut phone, now));
    assert_eq!(event_seqs(&packets), vec![1]);
    assert_eq!(state_seqs(&packets), vec![1]);
}

/// The latest state goes again on each new link, even acknowledged; a phone
/// that never had one has nothing to say.
#[test]
fn a_new_link_sends_the_latest_state_if_there_is_one() {
    let mut phone = joined(Ms(0), patient());
    let now = reopen(&mut phone, Ms(1));
    phone.received(now, &welcome(4, 1, 0, 0));
    assert_eq!(
        state_seqs(&sent(&drain(&mut phone, now))),
        Vec::<u32>::new()
    );

    let mut phone = joined(Ms(0), patient());
    phone.set_state(&3).unwrap();
    drain(&mut phone, Ms(1));
    let acked = Packet {
        ack: Some(Ack {
            events: 0,
            state: 1,
            ..Ack::default()
        }),
        ..from_host(1, 2)
    };
    phone.received(Ms(2), &encode(&acked));
    let now = reopen(&mut phone, Ms(3));
    phone.received(now, &welcome(4, 1, 0, 0));
    assert_eq!(state_seqs(&sent(&drain(&mut phone, now))), vec![1]);
}

/// An acknowledged state stops going; only the latest acknowledges it.
#[test]
fn only_the_latest_state_is_acknowledged() {
    let timing = Timing::default();
    let mut phone = joined(Ms(0), timing);
    phone.set_state(&1).unwrap();
    phone.set_state(&2).unwrap();
    assert_eq!(state_seqs(&sent(&drain(&mut phone, Ms(1)))), vec![2]);
    let ack = |state| {
        encode(&Packet {
            ack: Some(Ack {
                events: 0,
                state,
                ..Ack::default()
            }),
            ..from_host(1, 2)
        })
    };
    phone.received(Ms(2), &ack(1));
    assert_eq!(state_seqs(&sent(&drain(&mut phone, Ms(251)))), vec![2]);
    phone.received(Ms(252), &ack(2));
    assert_eq!(
        state_seqs(&sent(&drain(&mut phone, Ms(600)))),
        Vec::<u32>::new()
    );
}

#[test]
fn a_held_state_is_named_by_its_number() {
    let mut phone = joined(Ms(0), Timing::default());
    phone.set_state(&1).unwrap();
    phone.set_state(&2).unwrap();
    let packets = sent(&drain(&mut phone, Ms(1)));
    assert_eq!(packets[0].holding, Some(2));
}

/// The host forgot some events it had for this phone: wait for the next one
/// it has, never for ever. A lower number changes nothing.
#[test]
fn a_welcome_skips_only_forward() {
    let five = postcard::to_allocvec(&5_u32).unwrap();
    let mut phone = joined(Ms(0), Timing::default());
    let now = reopen(&mut phone, Ms(1));
    phone.received(now, &welcome(4, 1, 0, 3));
    drain(&mut phone, now);
    let next = Packet {
        events: vec![event(3, five.clone())],
        ..from_host(1, now.0)
    };
    phone.received(now, &encode(&next));
    assert_eq!(has(&drain(&mut phone, now), &ClientOutput::Event(5)), 1);

    let now = reopen(&mut phone, now.plus(1));
    phone.received(now, &welcome(4, 1, 0, 1));
    drain(&mut phone, now);
    let old = Packet {
        events: vec![event(3, five)],
        ..from_host(1, now.0)
    };
    phone.received(now, &encode(&old));
    assert_eq!(
        has(&drain(&mut phone, now), &ClientOutput::Event(5)),
        0,
        "already delivered"
    );
}

/// A piece kept across a cut is not lost when the host starts where we are.
#[test]
fn a_welcome_at_the_same_number_keeps_the_pieces() {
    let bytes = postcard::to_allocvec(&300_u32).unwrap();
    let (head, tail) = bytes.split_at(1);
    let part = |index: u16, data: &[u8]| wire::EventPart {
        seq: 0,
        index,
        count: 2,
        phase: 0,
        at: None,
        data: data.to_vec(),
    };
    let mut phone = joined(Ms(0), Timing::default());
    let first = Packet {
        events: vec![part(0, head)],
        ..from_host(1, 1)
    };
    phone.received(Ms(1), &encode(&first));
    let now = reopen(&mut phone, Ms(2));
    phone.received(now, &welcome(4, 1, 0, 0));
    drain(&mut phone, now);
    let second = Packet {
        events: vec![part(1, tail)],
        ..from_host(1, now.0)
    };
    phone.received(now, &encode(&second));
    assert_eq!(has(&drain(&mut phone, now), &ClientOutput::Event(300)), 1);
}

/// Another run of the host: what it said is void and both numberings start
/// from what the new welcome says, but what the player did still goes.
#[test]
fn a_new_run_of_the_host_voids_what_it_said_and_keeps_what_waits() {
    let five = postcard::to_allocvec(&5_u32).unwrap();
    let mut phone = joined(Ms(0), Timing::default());
    phone.send(&1).unwrap();
    let old = Packet {
        events: vec![event(0, five.clone()), event(1, five.clone())],
        state: vec![state(4, five.clone())],
        ..from_host(1, 1)
    };
    phone.received(Ms(1), &encode(&old));
    drain(&mut phone, Ms(1));

    let now = reopen(&mut phone, Ms(2));
    phone.received(now, &welcome(4, 2, 9, 1));
    let outputs = drain(&mut phone, now);
    assert_eq!(has(&outputs, &ClientOutput::Reset), 1);
    assert_eq!(event_seqs(&sent(&outputs)), vec![9], "what waited goes on");

    let new = Packet {
        events: vec![event(1, postcard::to_allocvec(&6_u32).unwrap())],
        state: vec![state(1, postcard::to_allocvec(&7_u32).unwrap())],
        ..from_host(2, now.0)
    };
    phone.received(now, &encode(&new));
    let outputs = drain(&mut phone, now);
    assert_eq!(has(&outputs, &ClientOutput::Event(6)), 1);
    assert_eq!(has(&outputs, &ClientOutput::State(7)), 1);

    phone.send(&2).unwrap();
    assert_eq!(event_seqs(&sent(&drain(&mut phone, now.plus(1)))), vec![10]);
}

/// With nothing else to do, the deadline is when the resend is due.
#[test]
fn the_next_deadline_follows_what_waits_for_a_resend() {
    let timing = Timing {
        heartbeat: 100_000,
        absent: 200_000,
        ..Timing::default()
    };
    let resend = 250;
    let mut phone = joined(Ms(0), timing);
    phone.send(&1).unwrap();
    drain(&mut phone, Ms(10));
    assert_eq!(phone.next_deadline(), Some(Ms(10 + resend)));

    let mut phone = joined(Ms(0), timing);
    phone.set_state(&1).unwrap();
    drain(&mut phone, Ms(10));
    assert_eq!(phone.next_deadline(), Some(Ms(10 + resend)));

    let mut phone = joined(Ms(0), timing);
    assert_eq!(phone.next_deadline(), Some(Ms(100_000)));
    phone.send(&1).unwrap();
    assert_eq!(phone.next_deadline(), Some(Ms(0)), "due now, never sent");
}

fn echoed(host_clock: u64, sent: u64) -> Vec<u8> {
    encode(&Packet {
        echo: Some(wire::Echo { sent, held: 0 }),
        ..from_host(1, host_clock)
    })
}

/// The host's clock when a packet left is half a round trip behind it.
#[test]
fn the_hosts_clock_is_its_stamp_plus_half_the_trip() {
    let mut phone = joined(Ms(0), Timing::default());
    assert_eq!(phone.host_time(Ms(1_000)), None);
    // Out at 1 000, back at 1 100, stamped 5 050 by the host: its clock was
    // 5 100 when the packet arrived.
    phone.received(Ms(1_100), &echoed(5_050, 1_000));
    assert_eq!(phone.host_time(Ms(1_100)), Some(Ms(5_100)));
    assert_eq!(phone.host_time(Ms(1_200)), Some(Ms(5_200)));
}

/// The sample with the smallest trip tells the clock, among the last 16.
#[test]
fn the_best_of_the_last_sixteen_samples_tells_the_clock() {
    let mut phone = joined(Ms(0), Timing::default());
    let mut now = 1_000;
    // Round trip 10, host ahead by 10 000.
    phone.received(Ms(now + 10), &echoed(now + 10_005, now));
    for _ in 0..15 {
        now += 100;
        // Round trip 50, host ahead by 20 000.
        phone.received(Ms(now + 50), &echoed(now + 20_025, now));
    }
    assert_eq!(phone.host_time(Ms(10_000)), Some(Ms(20_000)));
    now += 100;
    phone.received(Ms(now + 50), &echoed(now + 20_025, now));
    assert_eq!(phone.host_time(Ms(10_000)), Some(Ms(30_000)));
}

#[test]
fn a_new_link_measures_from_zero() {
    let mut phone = joined(Ms(0), Timing::default());
    phone.received(Ms(1_100), &echoed(1_100, 1_000));
    assert_eq!(phone.rtt(), Some(100));
    let now = reopen(&mut phone, Ms(1_200));
    phone.received(now, &welcome(4, 1, 0, 0));
    assert_eq!(phone.rtt(), None);
    assert_eq!(phone.host_time(now), None);
}
