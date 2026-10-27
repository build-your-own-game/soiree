//! The real host shell and real clients on a seeded in-process network.
use super::*;
use salon_core::{Caps, ClientConfig, Epoch, HostConfig, Limits, Timing, Token, TvKey};
use salon_host::driver::Driver;
use salon_host::simulation::{Faults, Network};

fn run(seed: u64) -> String {
    let timing = Timing {
        heartbeat: 10,
        silent: 40,
        absent: 90,
        handshake: 120,
        resend_min: 5,
        resend_max: 20,
        connect: 120,
        retry_first: 5,
        retry_max: 20,
        healthy: 50,
    };
    let game = Taps {
        join: Join {
            url: "http://localhost/play".into(),
            qr: salon_core::Qr::default(),
        },
        tvs: vec![],
        game: game_core::Game::default(),
        recording: None,
    };
    let config = HostConfig {
        game: VERSION,
        epoch: Epoch(1),
        tv_key: TvKey([7; 16]),
        timing,
        limits: Limits::default(),
        known: vec![],
    };
    let mut network = Network::new(
        seed,
        Faults {
            loss: 1,
            duplicate: 2,
            jitter: 3,
        },
        Driver::new(Ms(0), config, 100, game),
    );
    for index in 0_u8..3 {
        network.add_phone(
            ClientConfig {
                game: VERSION,
                kind: Kind::Phone,
                token: Token([index; 16]),
                name: None,
                tv_key: None,
                seed: seed.wrapping_add(u64::from(index)),
                timing,
                limits: Limits::default(),
            },
            Caps::STREAM,
        );
    }
    assert!(
        network.run_until(1000, |n| n
            .phones
            .iter()
            .all(|phone| phone.client.status() == salon_core::Status::Connected)),
        "connect seed {seed}"
    );
    let first = network.phones[0].client.device().unwrap().0;
    network.phones[0].client.send(&Up::Tap).unwrap();
    assert!(
        network.run_until(1000, |n| n
            .driver
            .game()
            .game
            .players()
            .get(&first)
            .is_some_and(|seat| seat.taps == 1)),
        "tap seed {seed}"
    );
    network.cut(0);
    network.freeze(1);
    network.run_checked(150, |n| {
        assert_eq!(
            n.driver.game().game.players().len(),
            3,
            "capacity seed {seed}"
        );
    });
    network.wake(1);
    assert!(
        network.run_until(1000, |n| n
            .phones
            .iter()
            .all(|phone| phone.client.status() == salon_core::Status::Connected)),
        "return seed {seed}"
    );
    assert_eq!(network.phones[0].client.device().unwrap().0, first);
    assert_eq!(
        network.driver.game().game.players()[&first].taps,
        1,
        "duplicate tap seed {seed}"
    );
    game_core::replay::fingerprint(&network.driver.game().game).unwrap()
}

#[test]
fn whole_host_is_deterministic_across_thousands_of_network_seeds() {
    let seeds = std::env::var("SOIREE_SIM_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2048);
    for seed in 0..seeds {
        assert_eq!(run(seed), run(seed), "whole host seed {seed}");
    }
}

#[test]
fn a_versioned_journal_replays_the_complete_state() {
    let journal = salon_host::journal::Journal::<game_core::replay::Header, game_core::replay::Entry>::from_reader(
        std::io::Cursor::new(include_bytes!("../tests/replays/session.jsonl"))).unwrap();
    game_core::replay::replay(&journal.header, &journal.entries, &journal.fingerprint).unwrap();
    assert!(game_core::replay::replay(&journal.header, &journal.entries, "wrong state").is_err());
}

#[test]
fn simultaneous_arrivals_never_exceed_capacity() {
    let game = Taps {
        join: Join {
            url: "http://localhost/play".into(),
            qr: salon_core::Qr::default(),
        },
        tvs: vec![],
        game: game_core::Game::default(),
        recording: None,
    };
    let config = HostConfig {
        game: VERSION,
        epoch: Epoch(1),
        tv_key: TvKey([7; 16]),
        timing: Timing::default(),
        limits: Limits::default(),
        known: vec![],
    };
    let mut network = Network::new(1, Faults::default(), Driver::new(Ms(0), config, 100, game));
    for index in 0_u8..9 {
        network.add_phone(
            ClientConfig {
                game: VERSION,
                kind: Kind::Phone,
                token: Token([index; 16]),
                name: None,
                tv_key: None,
                seed: u64::from(index),
                timing: Timing::default(),
                limits: Limits::default(),
            },
            Caps::STREAM,
        );
    }
    network.run(100);
    assert_eq!(
        network.driver.game().game.players().len(),
        game_core::MAX_PLAYERS
    );
    assert_eq!(
        network
            .phones
            .iter()
            .filter(|phone| phone.client.status() == salon_core::Status::Connected)
            .count(),
        game_core::MAX_PLAYERS
    );
    assert!(
        network
            .phones
            .iter()
            .any(|phone| matches!(phone.client.status(), salon_core::Status::Refused(_)))
    );
}
