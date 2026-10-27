//! Salon's guarantees (crate documentation), each played on the simulated
//! network of `net/`. The item of `CERTIFICATION.md` each covers is named.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] functions: a phone that never joins stops the test with its message"
)]

mod net;

use net::{Down, Faults, Net, Up, phone_config};
use salon_core::{Caps, DeviceEvent, DeviceId, Kind, Ms, Presence, Refusal, Status, Why};

fn device_of(net: &Net, phone: usize) -> DeviceId {
    net.phones[phone].client.device().expect("welcomed")
}

fn joined(net: &mut Net, phone: usize) -> DeviceId {
    assert!(
        net.run_until(5_000, |net| net.phones[phone].client.status()
            == Status::Connected),
        "phone {phone} never connected: {:?}",
        net.phones[phone].statuses
    );
    device_of(net, phone)
}

#[test]
fn a_phone_joins_and_both_sides_talk() {
    let mut net = Net::new(1, Faults::default());
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    assert!(matches!(
        net.happened.first(),
        Some((_, DeviceEvent::Arrived { kind: Kind::Phone, name: Some(name), .. })) if name.as_str() == "Phone 1"
    ));

    net.host
        .show(device, &Down::View(1, vec![1, 2, 3]))
        .unwrap();
    net.host.send(device, &Down::Note(5)).unwrap();
    net.phones[phone].client.send(&Up::Answer(42)).unwrap();
    net.run(200);

    assert_eq!(net.phones[phone].states, vec![Down::View(1, vec![1, 2, 3])]);
    assert_eq!(net.phones[phone].events, vec![Down::Note(5)]);
    assert_eq!(net.events_from(device), vec![Up::Answer(42)]);
}

#[test]
fn events_arrive_once_and_in_order_through_a_bad_network() {
    // 30 % loss, 10 % duplicates, 80 ms of jitter, each way.
    let mut net = Net::new(
        2,
        Faults {
            loss: 30,
            duplicate: 10,
            jitter: 80,
        },
    );
    let phone = net.add_phone(phone_config(2, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    for n in 0..50 {
        net.phones[phone].client.send(&Up::Answer(n)).unwrap();
        net.host.send(device, &Down::Note(n)).unwrap();
        net.run(40);
    }
    net.run(5_000);
    let want: Vec<Up> = (0..50).map(Up::Answer).collect();
    assert_eq!(net.events_from(device), want);
    let want: Vec<Down> = (0..50).map(Down::Note).collect();
    assert_eq!(net.phones[phone].events, want);
}

#[test]
fn the_latest_state_wins_and_a_big_one_is_cut_to_fit() {
    let mut net = Net::new(
        3,
        Faults {
            loss: 20,
            duplicate: 5,
            jitter: 50,
        },
    );
    let phone = net.add_phone(phone_config(3, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    // 20 KiB: about twenty packets of 1 100 bytes.
    let big: Vec<u8> = (0..20_000u32).map(|n| (n % 251) as u8).collect();
    for version in 1..=5 {
        net.host
            .show(device, &Down::View(version, big.clone()))
            .unwrap();
        net.run(10);
    }
    net.run(3_000);
    let states = &net.phones[phone].states;
    assert_eq!(states.last(), Some(&Down::View(5, big)));
    // Never an older state after a newer one.
    let versions: Vec<u32> = states
        .iter()
        .map(|state| match state {
            Down::View(version, _) => *version,
            Down::Note(_) => 0,
        })
        .collect();
    assert!(
        versions.windows(2).all(|pair| pair[0] < pair[1]),
        "{versions:?}"
    );
}

/// C2: a short cut, and the phone is back in its seat on its own, with
/// nothing lost and nothing doubled.
#[test]
fn a_cut_phone_comes_back_as_itself_without_losing_an_event() {
    let mut net = Net::new(4, Faults::default());
    let phone = net.add_phone(phone_config(4, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].client.send(&Up::Answer(1)).unwrap();
    // Cut before the event can arrive; queue more while away.
    net.cut(phone);
    net.phones[phone].client.send(&Up::Answer(2)).unwrap();
    net.host.send(device, &Down::Note(9)).unwrap();
    assert!(
        net.run_until(10_000, |net| net.phones[phone].client.status()
            == Status::Connected
            && net.events_from(device).len() == 2)
    );
    net.run(500);
    assert_eq!(device_of(&net, phone), device);
    assert_eq!(net.events_from(device), vec![Up::Answer(1), Up::Answer(2)]);
    assert_eq!(net.phones[phone].events, vec![Down::Note(9)]);
    assert_eq!(
        net.count(|event| matches!(
            event,
            DeviceEvent::Left {
                why: Why::Closed,
                ..
            }
        )),
        1
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Back { .. })),
        1
    );
    assert_eq!(net.phones[phone].resets, 0);
}

/// C3: the Wi-Fi dies without a word. The host notices on its own, and so
/// does the phone, which reconnects once the network is back.
#[test]
fn silence_is_noticed_on_both_sides() {
    let mut net = Net::new(5, Faults::default());
    let phone = net.add_phone(phone_config(5, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].frozen = true;
    let frozen_at = net.now;
    assert!(net.run_until(20_000, |net| net.count(|event| matches!(
        event,
        DeviceEvent::Left {
            why: Why::Silent,
            ..
        }
    )) == 1));
    let noticed = net.now.since(frozen_at);
    assert!(noticed <= 7_000, "host noticed after {noticed} ms");
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.presence == Presence::Absent)
    );
    // The phone's own watchdog drops the dead socket without any close.
    assert!(net.run_until(10_000, |net| {
        net.phones[phone].statuses.contains(&Status::Reconnecting)
    }));
    net.phones[phone].frozen = false;
    assert!(
        net.run_until(10_000, |net| net.phones[phone].client.status()
            == Status::Connected)
    );
    assert_eq!(device_of(&net, phone), device);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Back { .. })),
        1
    );
}

/// C5: two tabs for the same player. The newest wins, the old one stops,
/// and nobody bounces.
#[test]
fn a_second_tab_takes_over_and_the_first_stays_put() {
    let mut net = Net::new(6, Faults::default());
    let first = net.add_phone(phone_config(6, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, first);
    let second = net.add_phone(phone_config(6, Kind::Phone), Caps::DATAGRAM);
    assert_eq!(joined(&mut net, second), device);
    net.run(30_000);
    assert_eq!(net.phones[first].client.status(), Status::Replaced);
    assert_eq!(net.phones[second].client.status(), Status::Connected);
    assert_eq!(
        net.phones[first].connects, 1,
        "the replaced tab must not reconnect on its own"
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { .. })),
        0
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Arrived { .. })),
        1
    );
    // "Play here" on the first tab takes the seat back, once.
    net.phones[first].client.resume();
    net.run(5_000);
    assert_eq!(net.phones[first].client.status(), Status::Connected);
    assert_eq!(net.phones[second].client.status(), Status::Replaced);
}

/// C7: the host crashes and starts again. Phones reconnect on their own,
/// learn that what it said is void, and keep their identity.
#[test]
fn a_restarted_host_gets_its_phones_back() {
    let mut net = Net::new(7, Faults::default());
    let phone = net.add_phone(phone_config(7, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.run(500);
    let known = net.host.known();
    net.restart_host(2, known);
    assert!(net.run_until(15_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    assert_eq!(device_of(&net, phone), device);
    // The new run never heard of it: an arrival, under its old identity.
    // Then it keeps numbering above it.
    let arrivals =
        net.count(|event| matches!(event, DeviceEvent::Arrived { device: d, .. } if *d == device));
    assert_eq!(arrivals, 2, "once per run");
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Back { .. })),
        0
    );
    let other = net.add_phone(phone_config(8, Kind::Phone), Caps::DATAGRAM);
    assert!(joined(&mut net, other).0 > device.0);
}

#[test]
fn a_restart_without_memory_makes_new_devices() {
    let mut net = Net::new(8, Faults::default());
    let phone = net.add_phone(phone_config(9, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.restart_host(3, Vec::new());
    assert!(net.run_until(15_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Arrived { .. })),
        2
    );
}

#[test]
fn a_graceful_shutdown_tells_the_phones() {
    let mut net = Net::new(9, Faults::default());
    let phone = net.add_phone(phone_config(10, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.host.shutdown();
    net.pump_host();
    net.run(100);
    assert!(net.phones[phone].statuses.contains(&Status::HostGone));
}

/// C8: clear refusals, never a ghost lobby.
#[test]
fn refusals_are_clear_and_final() {
    let mut net = Net::new(10, Faults::default());
    let mut wrong = phone_config(11, Kind::Phone);
    wrong.game += 1;
    let old = net.add_phone(wrong, Caps::DATAGRAM);
    let mut fake_tv = phone_config(12, Kind::Tv);
    fake_tv.tv_key = None;
    let tv = net.add_phone(fake_tv, Caps::DATAGRAM.local());
    net.run(5_000);
    assert_eq!(
        net.phones[old].client.status(),
        Status::Refused(Refusal::Version)
    );
    assert_eq!(
        net.phones[tv].client.status(),
        Status::Refused(Refusal::NotAllowed)
    );
    assert_eq!(net.phones[old].connects, 1);
    assert_eq!(net.happened, Vec::new());

    net.host.close();
    let late = net.add_phone(phone_config(13, Kind::Phone), Caps::DATAGRAM);
    net.run(2_000);
    assert_eq!(
        net.phones[late].client.status(),
        Status::Refused(Refusal::Closed)
    );
}

#[test]
fn the_room_has_a_limit() {
    let mut net = Net::new(11, Faults::default());
    let max = salon_core::Limits::default().max_devices;
    for token in 0..max {
        net.add_phone(
            phone_config(u8::try_from(token + 20).unwrap(), Kind::Phone),
            Caps::DATAGRAM,
        );
    }
    net.run(3_000);
    let extra = net.add_phone(phone_config(200, Kind::Phone), Caps::DATAGRAM);
    net.run(2_000);
    assert_eq!(
        net.phones[extra].client.status(),
        Status::Refused(Refusal::Full)
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Arrived { .. })),
        max
    );
}

#[test]
fn the_tv_joins_with_its_key() {
    let mut net = Net::new(12, Faults::default());
    let tv = net.add_phone(phone_config(14, Kind::Tv), Caps::STREAM.local());
    joined(&mut net, tv);
    assert!(matches!(
        net.happened.first(),
        Some((_, DeviceEvent::Arrived { kind: Kind::Tv, .. }))
    ));
    // E4: the same key and token, replayed from the network, are refused.
    let replay = net.add_phone(phone_config(14, Kind::Tv), Caps::STREAM);
    net.run(2_000);
    assert_eq!(
        net.phones[replay].client.status(),
        Status::Refused(Refusal::NotAllowed)
    );
    assert_eq!(net.phones[tv].client.status(), Status::Connected);
}

#[test]
fn a_player_who_quits_leaves_for_that_reason() {
    let mut net = Net::new(13, Faults::default());
    let phone = net.add_phone(phone_config(15, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.phones[phone].client.quit();
    net.run(1_000);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { why: Why::Quit, .. })),
        1
    );
    assert_eq!(net.phones[phone].client.status(), Status::Quit);
    net.run(10_000);
    assert_eq!(net.phones[phone].connects, 1);
}

/// E3: a phone that floods the host slows itself down, not the others.
#[test]
fn a_flooding_phone_does_not_slow_down_the_others() {
    let mut net = Net::new(14, Faults::default());
    let flood = net.add_phone(phone_config(16, Kind::Phone), Caps::DATAGRAM);
    let calm = net.add_phone(phone_config(17, Kind::Phone), Caps::DATAGRAM);
    let flooder = joined(&mut net, flood);
    let calm_device = joined(&mut net, calm);
    let mut refused = 0;
    for n in 0..2_000 {
        if net.phones[flood].client.send(&Up::Answer(n)).is_err() {
            refused += 1;
        }
    }
    assert!(
        refused > 0,
        "the flooding phone's own queue must say it is full"
    );
    let sent_at = net.now;
    net.phones[calm].client.send(&Up::Answer(1)).unwrap();
    assert!(net.run_until(1_000, |net| net.events_from(calm_device)
        == vec![Up::Answer(1)]));
    assert!(
        net.now.since(sent_at) < 100,
        "the calm phone waited {} ms",
        net.now.since(sent_at)
    );
    // The flooder gets its events through, at the rate allowed, in order.
    net.run(5_000);
    let got = net.events_from(flooder);
    assert!(
        got.len() <= 60 + 30 * 6 + 1,
        "{} events in about 6 s",
        got.len()
    );
    assert!(
        got.iter()
            .zip(0..)
            .all(|(event, n)| *event == Up::Answer(n))
    );
}

/// Gamepads and the keyboard are devices like the others.
#[test]
fn a_gamepad_is_a_device_like_the_others() {
    let mut net = Net::new(15, Faults::default());
    let pad = net.host.add_local(net.now, Kind::Gamepad, None);
    net.host.local_state(net.now, pad, Up::Pad(1, -1));
    net.host.local_unplugged(net.now, pad);
    net.host.local_plugged(net.now, pad);
    net.pump_host();
    let kinds: Vec<&DeviceEvent<Up>> = net.happened.iter().map(|(_, event)| event).collect();
    assert!(matches!(
        kinds[0],
        DeviceEvent::Arrived {
            kind: Kind::Gamepad,
            ..
        }
    ));
    assert!(matches!(
        kinds[1],
        DeviceEvent::State {
            state: Up::Pad(1, -1),
            ..
        }
    ));
    assert!(matches!(
        kinds[2],
        DeviceEvent::Left {
            why: Why::Closed,
            ..
        }
    ));
    assert!(matches!(kinds[3], DeviceEvent::Back { .. }));
    assert!(net.host.show(pad, &Down::Note(1)).is_err());
}

/// E5: every event carries the phase its sender had seen.
#[test]
fn an_event_carries_the_phase_its_sender_saw() {
    let mut net = Net::new(16, Faults::default());
    let phone = net.add_phone(phone_config(18, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.host.set_phase(3);
    net.run(500);
    net.phones[phone].client.send(&Up::Answer(1)).unwrap();
    net.run(100);
    assert!(net.happened.iter().any(|(_, event)| matches!(
        event,
        DeviceEvent::Event { device: d, phase: 3, .. } if *d == device
    )));
}

#[test]
fn round_trips_and_the_host_clock_are_measured() {
    let mut net = Net::new(
        17,
        Faults {
            loss: 0,
            duplicate: 0,
            jitter: 20,
        },
    );
    let phone = net.add_phone(phone_config(19, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.run(2_000);
    let rtt = net
        .host
        .device(device)
        .and_then(|info| info.rtt)
        .expect("measured");
    assert!(rtt <= 60, "{rtt}");
    let host_time = net.phones[phone]
        .client
        .host_time(net.now)
        .expect("measured");
    assert!(
        host_time.0.abs_diff(net.now.0) <= 30,
        "{host_time:?} against {:?}",
        net.now
    );
}

#[test]
fn a_held_state_is_known_to_be_held() {
    let mut net = Net::new(18, Faults::default());
    let phone = net.add_phone(phone_config(20, Kind::App), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].client.set_state(&Up::Pad(1, 0)).unwrap();
    net.run(2_000);
    let held = net
        .host
        .device(device)
        .and_then(|info| info.held_at)
        .expect("held");
    // Refreshed by heartbeats, without resending the state.
    assert!(net.now.since(held) <= 300);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::State { .. })),
        1
    );
}

#[test]
fn events_sent_before_the_first_welcome_count() {
    let mut net = Net::new(19, Faults::default());
    let phone = net.add_phone(phone_config(21, Kind::Phone), Caps::DATAGRAM);
    net.phones[phone].client.send(&Up::Answer(1)).unwrap();
    let device = joined(&mut net, phone);
    net.run(200);
    assert_eq!(net.events_from(device), vec![Up::Answer(1)]);
}

#[test]
fn time_never_goes_back() {
    let mut net = Net::new(20, Faults::default());
    let phone = net.add_phone(phone_config(22, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    // A clock that jumps back must not wake every timeout at once.
    net.host.tick(Ms(0));
    net.run(100);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { .. })),
        0
    );
}

/// E4: a phone's token cannot turn into the TV, even with the key and from
/// this computer; the phone keeps its place.
#[test]
fn a_phone_token_never_becomes_the_tv() {
    let mut net = Net::new(21, Faults::default());
    let phone = net.add_phone(phone_config(23, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    let mut as_tv = phone_config(23, Kind::Tv);
    as_tv.tv_key = Some(net::TV_KEY);
    let impostor = net.add_phone(as_tv, Caps::STREAM.local());
    net.run(2_000);
    assert_eq!(
        net.phones[impostor].client.status(),
        Status::Refused(Refusal::NotAllowed)
    );
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.kind == Kind::Phone && info.presence == Presence::Present)
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { .. })),
        0
    );
}

/// A `Hello` repeated on a welcomed link (its welcome was lost) changes
/// nothing: same device, no event.
#[test]
fn a_repeated_hello_changes_nothing() {
    let mut net = Net::new(22, Faults::default());
    let phone = net.add_phone(phone_config(24, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    let before = net.happened.len();
    let link = net.phones[phone].link.expect("connected");
    // The client's own first packet was a Hello; replay a fresh one by
    // making a twin client produce it, then feed it on the same link.
    let mut twin: salon_core::Client<Up, Down> =
        salon_core::Client::new(net.now, phone_config(24, Kind::Phone));
    // The first poll asks to connect: the test opens by hand just after.
    let _: Option<salon_core::ClientOutput<Down>> = twin.poll(net.now);
    twin.opened(net.now, Caps::DATAGRAM);
    let hello = std::iter::from_fn(|| twin.poll(net.now))
        .find_map(|output| {
            if let salon_core::ClientOutput::Transmit(bytes) = output {
                Some(bytes)
            } else {
                None
            }
        })
        .expect("a hello");
    for _ in 0..3 {
        net.host.received(net.now, link, &hello);
    }
    net.run(500);
    assert_eq!(device_of(&net, phone), device);
    assert_eq!(net.happened.len(), before);
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
}

/// C4: reloading the page is a new client with the stored token and no
/// memory: it gets the same device back.
#[test]
fn a_reloaded_page_gets_its_device_back() {
    let mut net = Net::new(23, Faults::default());
    let page = net.add_phone(phone_config(25, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, page);
    net.cut(page);
    net.phones[page].client.quit();
    let reloaded = net.add_phone(phone_config(25, Kind::Phone), Caps::DATAGRAM);
    assert_eq!(joined(&mut net, reloaded), device);
    assert_eq!(net.phones[reloaded].resets, 0);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Arrived { .. })),
        1
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Back { device: d } if *d == device)),
        1
    );
}

/// The TV's key may change (a host restarted without its data folder): the
/// page sets the new one and is taken back.
#[test]
fn a_tv_refused_for_an_old_key_comes_back_with_the_new_one() {
    let mut net = Net::new(24, Faults::default());
    let mut stale = phone_config(26, Kind::Tv);
    stale.tv_key = Some(salon_core::TvKey([1; 16]));
    let tv = net.add_phone(stale, Caps::STREAM.local());
    net.run(2_000);
    assert_eq!(
        net.phones[tv].client.status(),
        Status::Refused(Refusal::NotAllowed)
    );
    net.phones[tv].client.set_tv_key(net::TV_KEY);
    net.phones[tv].client.resume();
    joined(&mut net, tv);
}

/// E4 the other way: the TV's token cannot come back as a phone; the TV
/// keeps its place.
#[test]
fn the_tv_token_never_becomes_a_phone() {
    let mut net = Net::new(25, Faults::default());
    let tv = net.add_phone(phone_config(27, Kind::Tv), Caps::STREAM.local());
    let device = joined(&mut net, tv);
    let impostor = net.add_phone(phone_config(27, Kind::Phone), Caps::DATAGRAM);
    net.run(2_000);
    assert_eq!(
        net.phones[impostor].client.status(),
        Status::Refused(Refusal::NotAllowed)
    );
    assert_eq!(net.phones[tv].client.status(), Status::Connected);
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.kind == Kind::Tv)
    );
}

/// A TV `Hello` on a phone's welcomed link changes nothing: it stays that
/// phone.
#[test]
fn a_welcomed_phone_cannot_turn_into_the_tv() {
    let mut net = Net::new(26, Faults::default());
    let phone = net.add_phone(phone_config(28, Kind::Phone), Caps::DATAGRAM.local());
    let device = joined(&mut net, phone);
    let link = net.phones[phone].link.expect("connected");
    let mut as_tv = phone_config(28, Kind::Tv);
    as_tv.tv_key = Some(net::TV_KEY);
    let mut forger: salon_core::Client<Up, Down> = salon_core::Client::new(net.now, as_tv);
    let _: Option<salon_core::ClientOutput<Down>> = forger.poll(net.now);
    forger.opened(net.now, Caps::DATAGRAM);
    let hello = std::iter::from_fn(|| forger.poll(net.now))
        .find_map(|output| {
            if let salon_core::ClientOutput::Transmit(bytes) = output {
                Some(bytes)
            } else {
                None
            }
        })
        .expect("a hello");
    net.host.received(net.now, link, &hello);
    net.run(500);
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.kind == Kind::Phone)
    );
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
}

/// iOS freezes a page in the background; when it comes back, `wake`
/// reconnects at once instead of waiting for the watchdog.
#[test]
fn a_page_that_wakes_up_reconnects_at_once() {
    let mut net = Net::new(27, Faults::default());
    let phone = net.add_phone(phone_config(29, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].frozen = true;
    // Asleep for 3 s: less than the watchdog's 6 s.
    net.run(3_000);
    net.phones[phone].frozen = false;
    let woke = net.now;
    net.phones[phone].client.wake(net.now);
    assert!(net.run_until(1_000, |net| net.phones[phone].connects == 2
        && net.phones[phone].client.status() == Status::Connected));
    assert!(net.now.since(woke) < 500, "{} ms", net.now.since(woke));
    assert_eq!(device_of(&net, phone), device);
}

/// With no host to answer, attempts space out up to 2 s and never stop.
#[test]
fn a_phone_without_host_keeps_trying_ever_more_calmly() {
    let mut net = Net::new(28, Faults::default());
    let phone = net.add_phone(phone_config(30, Kind::Phone), Caps::DATAGRAM);
    // Unreachable: every attempt times out.
    net.phones[phone].frozen = true;
    let mut attempts = Vec::new();
    for _ in 0..120 {
        net.run(1_000);
        attempts.push(net.phones[phone].connects);
    }
    let total = net.phones[phone].connects;
    // Each attempt waits for the 5 s connect timeout, then up to 2 s ± 25 %.
    assert!((15..=25).contains(&total), "{total} attempts in 120 s");
    // Still trying at the end.
    assert!(attempts[119] > attempts[100]);
}
