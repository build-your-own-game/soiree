//! An evening on the real host driver and the real clients, in seconds:
//! salon's memory depends on what happens, not on how long it takes, so the
//! time is simulated and the bytes counted, exactly: six laps, of ten minutes
//! here and of an hour with `SOIREE_EVENING_MINUTES=60`. The harness's
//! `--soak` watches a process for 30 real minutes; this looks closer and
//! plays longer. A real browser and a real socket stay its to see. Bytes
//! that only a forged packet makes salon hold are not reached here.

#![expect(clippy::expect_used, reason = "test: a seat below a constant is one")]
#![expect(
    clippy::indexing_slicing,
    reason = "test: every seat below PHONES was added"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: bounded seats and turns; a violation must fail the test"
)]

use std::collections::BTreeMap;

use salon_core::{
    Caps, ClientConfig, DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind, Limits, Ms, Timing,
    Token, TvKey,
};
use salon_host::Game;
use salon_host::driver::Driver;
use salon_host::simulation::{Counting, Faults, Moment, Network, evening, flat};

#[global_allocator]
static COUNTED: Counting = Counting;

const PHONES: usize = 8;
/// A party moves this often, simulated.
const TURN_MS: u64 = 20_000;
/// Seats kept all evening. The first never loses its link, for what a link
/// gathers while it lives; the others go through hundreds of reconnections, and
/// past them a new identity comes every few minutes.
const LOYAL: usize = 4;
/// What most taps carry, and every fourth one: more than a packet holds.
const SMALL: usize = 40;
const LARGE: usize = 6_000;
/// A phone gone this long is forgotten, as a game forgets a passing stranger.
const GRACE_MS: u64 = 120_000;
/// The end of each lap: everybody comes back, and a phone gone for good is
/// forgotten.
const CALM_MINUTES: u64 = 5;
/// What may move after the first lap: the capacity of a list.
const MARGIN: usize = 1024;

#[derive(Default)]
struct Party {
    scores: BTreeMap<DeviceId, u32>,
    gone: BTreeMap<DeviceId, Ms>,
    /// Keeps every event, which no game should: the leak this test must see.
    hoard: Option<Vec<Vec<u8>>>,
}

impl Game for Party {
    type Up = Vec<u8>;
    type Down = Vec<u8>;

    fn on(&mut self, now: Ms, event: DeviceEvent<Vec<u8>>, host: &mut Host<Vec<u8>, Vec<u8>>) {
        match event {
            DeviceEvent::Arrived { device, .. } => {
                self.scores.entry(device).or_default();
                let _: Result<(), _> = host.show(device, &Vec::new());
            }
            DeviceEvent::Back { device } => {
                self.gone.remove(&device);
            }
            DeviceEvent::Left { device, .. } => {
                self.gone.insert(device, now);
            }
            DeviceEvent::Event { device, event, .. } => {
                let score = self.scores.entry(device).or_default();
                *score = score.saturating_add(1);
                let _: Result<(), _> = host.show(device, &event);
                let _: Result<(), _> = host.send(device, &event);
                if let Some(hoard) = &mut self.hoard {
                    hoard.push(event);
                }
            }
            DeviceEvent::State { .. } => {}
        }
    }

    fn tick(&mut self, now: Ms, host: &mut Host<Vec<u8>, Vec<u8>>) {
        let forgotten: Vec<DeviceId> = self
            .gone
            .iter()
            .filter(|(_, since)| now.0.saturating_sub(since.0) > GRACE_MS)
            .map(|(device, _)| *device)
            .collect();
        for device in forgotten {
            self.gone.remove(&device);
            self.scores.remove(&device);
            host.forget(device);
        }
    }
}

fn phone(identity: u64) -> ClientConfig {
    let mut token = [0; 16];
    token[..8].copy_from_slice(&identity.to_le_bytes());
    ClientConfig {
        game: 1,
        kind: Kind::Phone,
        token: Token(token),
        name: None,
        tv_key: None,
        seed: identity,
        timing: Timing::default(),
        limits: Limits::default(),
    }
}

fn room(game: Party) -> Network<Party> {
    let config = HostConfig {
        game: 1,
        epoch: Epoch(1),
        tv_key: TvKey([0; 16]),
        timing: Timing::default(),
        limits: Limits::default(),
        known: Vec::new(),
    };
    let faults = Faults {
        loss: 5,
        duplicate: 5,
        jitter: 40,
    };
    let mut network = Network::new(7, faults, Driver::new(Ms(0), config, 17, game));
    for identity in 0..PHONES {
        network.add_phone(phone(identity as u64), Caps::DATAGRAM);
    }
    network
}

/// A throw of dice for this minute (`SplitMix64`).
fn dice(minute: u64) -> u64 {
    let z = minute.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Twenty seconds of a party: everybody taps, two phones have a bad moment, a
/// page reloads and a new identity takes a seat.
fn play(network: &mut Network<Party>, moment: Moment) {
    let turn = moment.step;
    if moment.calm {
        for seat in 0..PHONES {
            network.wake(seat);
            let _: Result<(), _> = network.phones[seat].client.send(&vec![0; SMALL]);
        }
        return;
    }
    for phone in &mut network.phones {
        if !phone.frozen {
            for tap in 0..16 {
                let size = if tap % 4 == 0 { LARGE } else { SMALL };
                // A full queue refuses a tap; the page says so and goes on.
                let _: Result<(), _> = phone.client.send(&vec![0; size]);
            }
        }
    }
    let throw = dice(turn);
    network.disturb(1..PHONES, throw, turn);
    network.disturb(1..PHONES, throw.rotate_left(17), turn);
    let loyal =
        1 + usize::try_from(throw.rotate_left(29) % (LOYAL as u64 - 1)).expect("below LOYAL");
    network.cut(loyal);
    // A reload: the same identity, a new page.
    let reloading = 2 + loyal % (LOYAL - 2);
    network.replace(reloading, phone(reloading as u64));
    let others = (PHONES - LOYAL) as u64;
    let seat = LOYAL + usize::try_from(throw.rotate_left(9) % others).expect("below PHONES");
    network.replace(seat, phone(PHONES as u64 + turn));
}

#[test]
fn an_evening_leaves_nothing_in_the_room() {
    let mut network = room(Party::default());
    let laps = evening(&mut network, TURN_MS, CALM_MINUTES, play);
    println!("{laps:?}");
    flat(&laps, MARGIN).unwrap();
}

/// The control: what this test exists for is seen, in a game that keeps every
/// event, at a rate no louder than a real one's.
#[test]
fn a_game_that_keeps_what_it_hears_is_caught() {
    let mut network = room(Party {
        hoard: Some(Vec::new()),
        ..Party::default()
    });
    let laps = evening(&mut network, TURN_MS, CALM_MINUTES, play);
    assert!(flat(&laps, MARGIN).is_err(), "{laps:?}");
}
