//! Salon's guarantees (crate documentation), each played on the simulated
//! network of `net/`. The item of `CERTIFICATION.md` each covers is named.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] functions: a phone that never joins stops the test with its message"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]

mod framing;
mod net;
#[expect(
    dead_code,
    reason = "the forged packets are shared with tests/client_edges.rs, which uses all of them"
)]
mod wire;

use net::{Down, Faults, Narrow, Net, Up, host_config, phone_config};
use salon_core::{
    Caps, ClientConfig, ClientSendError, DeviceEvent, DeviceId, Host, HostConfig, Kind, Limits, Ms,
    Presence, Refusal, SendError, Status, Timing, Why,
};

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

/// A page frozen for less than the host's patience, and told nothing: nothing
/// of it ran, what came waited in its socket, and the link never died.
#[test]
fn a_page_that_thaws_in_silence_keeps_its_link() {
    let mut net = Net::new(30, Faults::default());
    let phone = net.add_phone(phone_config(30, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].asleep = true;
    net.host.send(device, &Down::Note(1)).unwrap();
    net.run(3_000);
    assert_eq!(net.phones[phone].events, vec![]);
    net.phones[phone].asleep = false;
    net.stir();
    net.run(1_000);
    assert_eq!(net.phones[phone].events, vec![Down::Note(1)]);
    assert_eq!(net.phones[phone].connects, 1);
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
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

/// RTC owns 15 s then TCP owns 5 s; the core's default 5 s must not destroy
/// the first negotiation repeatedly before its shell can choose TCP.
#[test]
fn transport_phase_budgets_are_absolute_and_the_next_attempt_keeps_default() {
    use salon_core::{Client, ClientOutput};
    let mut client = Client::<Up, Down>::new(Ms(1_000), phone_config(31, Kind::Phone));
    assert!(matches!(
        client.poll(Ms(1_000)),
        Some(ClientOutput::Connect)
    ));
    client.connecting_for(Ms(1_000), 20_000);
    assert_eq!(client.next_deadline(), Some(Ms(21_000)));
    assert!(client.poll(Ms(6_000)).is_none());
    assert!(client.poll(Ms(15_999)).is_none());
    // The shell starts TCP after its own 15 s deadline, without progress
    // calls in between. The original 20 s total remains the same.
    client.connecting_for(Ms(16_000), 5_000);
    assert_eq!(client.next_deadline(), Some(Ms(21_000)));
    assert!(client.poll(Ms(20_999)).is_none());
    assert!(matches!(
        client.poll(Ms(21_000)),
        Some(ClientOutput::Disconnect)
    ));
    let retry = client.next_deadline().unwrap();
    assert!(matches!(client.poll(retry), Some(ClientOutput::Connect)));
    assert_eq!(client.next_deadline(), Some(retry.plus(5_000)));
}

#[test]
fn a_transport_budget_cannot_postpone_an_open_connections_handshake() {
    use salon_core::{Client, ClientOutput};
    let mut client = Client::<Up, Down>::new(Ms(0), phone_config(32, Kind::Phone));
    assert!(matches!(client.poll(Ms(0)), Some(ClientOutput::Connect)));
    client.opened(Ms(0), Caps::DATAGRAM);
    client.connecting_for(Ms(9_999), u64::MAX);
    while client.poll(Ms(9_999)).is_some() {}
    assert!(matches!(
        client.poll(Ms(10_000)),
        Some(ClientOutput::Disconnect)
    ));
}

/// The first packet a client with this configuration sends: its hello.
fn hello_of(config: salon_core::ClientConfig, now: Ms) -> Vec<u8> {
    use salon_core::{Client, ClientOutput};
    let mut client: Client<Up, Down> = Client::new(now, config);
    assert_eq!(client.poll(now), Some(ClientOutput::Connect));
    client.opened(now, Caps::DATAGRAM);
    std::iter::from_fn(|| client.poll(now))
        .find_map(|output| {
            if let ClientOutput::Transmit(bytes) = output {
                Some(bytes)
            } else {
                None
            }
        })
        .expect("a hello")
}

/// C5: a late copy of a hello, on the link that was just replaced, must not
/// take the device back from the link that replaced it.
#[test]
fn a_late_hello_on_a_replaced_link_changes_nothing() {
    let mut net = Net::new(29, Faults::default());
    let first = net.add_phone(phone_config(33, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, first);
    let old = net.phones[first].link.expect("connected");
    let second = net.add_phone(phone_config(33, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, second);
    let hello = hello_of(phone_config(33, Kind::Phone), net.now);
    net.host.received(net.now, old, &hello);
    net.run(30_000);
    assert_eq!(net.phones[first].client.status(), Status::Replaced);
    assert_eq!(net.phones[second].client.status(), Status::Connected);
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.presence == Presence::Present)
    );
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { .. })),
        0
    );
}

/// C5: a phone tries a link, its hello stays stuck in the socket, the network
/// comes back and the phone tries another. TCP then hands over the old hello,
/// before the old close. A device is not replaced by its own abandoned try: no
/// tab would be left to play.
#[test]
fn a_hello_stuck_on_an_abandoned_attempt_does_not_replace_the_link_after_it() {
    let mut net = Net::new(61, Faults::default());
    let phone = net.add_phone(phone_config(61, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.cut(phone);
    net.phones[phone].held = 3_000;
    assert!(net.run_until(600, |net| net.phones[phone].link.is_some()));
    net.run(1_600);
    let now = net.now;
    net.phones[phone].client.network_back(now);
    assert!(net.run_until(500, |net| net.phones[phone].client.status()
        == Status::Connected));
    assert_eq!(
        net.phones[phone].connects, 3,
        "the join, the stuck try, this one"
    );
    net.run(30_000);
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
    assert_eq!(net.phones[phone].connects, 3);
    assert!(
        net.host
            .device(device)
            .is_some_and(|info| info.presence == Presence::Present)
    );
}

/// The same when the page reloads: the hello of the page that left comes
/// behind the one of the page that took its place.
#[test]
fn a_hello_of_the_page_before_a_reload_does_not_replace_the_new_page() {
    let mut net = Net::new(62, Faults::default());
    let old = net.add_phone(phone_config(62, Kind::Phone), Caps::DATAGRAM);
    net.phones[old].held = 500;
    net.run(100);
    assert!(net.phones[old].link.is_some(), "its hello is on its way");
    net.leave(old);
    let new = net.add_phone(phone_config(62, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, new);
    net.run(30_000);
    assert_eq!(net.phones[new].client.status(), Status::Connected);
    assert_eq!(net.phones[new].connects, 1);
}

/// C5: the newest link wins, whichever hello comes first. The tab that loses
/// is told at once, and offers to play here.
#[test]
fn of_two_tabs_the_newest_link_wins_even_when_its_hello_comes_first() {
    let mut net = Net::new(63, Faults::default());
    let first = net.add_phone(phone_config(63, Kind::Phone), Caps::DATAGRAM);
    net.phones[first].held = 300;
    net.run(100);
    assert!(net.phones[first].link.is_some(), "its hello is on its way");
    let second = net.add_phone(phone_config(63, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, second);
    net.run(1_000);
    assert_eq!(net.phones[first].client.status(), Status::Replaced);
    assert_eq!(net.phones[second].client.status(), Status::Connected);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { .. })),
        0
    );
    // It can still take the device back, on a link newer than the other's.
    net.phones[first].client.resume();
    assert!(net.run_until(2_000, |net| net.phones[first].client.status()
        == Status::Connected));
    assert_eq!(device_of(&net, first), device);
    net.run(1_000);
    assert_eq!(net.phones[second].client.status(), Status::Replaced);
}

/// C5, found by chance: two tabs at once on a bad network, with their hellos
/// repeated and delayed. One of them plays, whatever the order of arrival;
/// they used to evict each other for good in 7 % of the runs.
#[test]
fn two_tabs_opened_together_leave_one_playing() {
    for seed in 0..200 {
        let mut net = Net::new(
            seed,
            Faults {
                loss: 20,
                duplicate: 10,
                jitter: 80,
            },
        );
        let first = net.add_phone(phone_config(34, Kind::Phone), Caps::DATAGRAM);
        let second = net.add_phone(phone_config(34, Kind::Phone), Caps::DATAGRAM);
        net.run(15_000);
        let statuses = [
            net.phones[first].client.status(),
            net.phones[second].client.status(),
        ];
        assert!(
            statuses.contains(&Status::Connected),
            "seed {seed}: {statuses:?}"
        );
    }
}

/// E3: once a link was told no, it has nothing more to say. A copy of its
/// hello must not make a device of it: the device would hold a seat for ever
/// on a link about to close, with no `Left`.
#[test]
fn a_hello_after_a_refusal_makes_no_device() {
    let mut net = Net::new(30, Faults::default());
    let now = net.now;
    let link = salon_core::LinkId(900);
    net.host.opened(now, link, Caps::DATAGRAM);
    let mut stale = phone_config(35, Kind::Phone);
    stale.game += 1;
    net.host.received(now, link, &hello_of(stale, now));
    net.host
        .received(now, link, &hello_of(phone_config(35, Kind::Phone), now));
    net.run(5_000);
    assert_eq!(net.host.devices().count(), 0);
    assert_eq!(net.happened, Vec::new());
}

/// A place that the first device of a full room gives back must go to the
/// next one: a copy of a hello on the refused link does not take it.
#[test]
fn a_late_hello_on_a_link_refused_for_a_full_room_takes_no_seat() {
    let mut net = Net::new(31, Faults::default());
    let max = salon_core::Limits::default().max_devices;
    for token in 0..max {
        net.add_phone(
            phone_config(u8::try_from(token + 40).unwrap(), Kind::Phone),
            Caps::DATAGRAM,
        );
    }
    net.run(3_000);
    let now = net.now;
    let refused = salon_core::LinkId(900);
    let hello = hello_of(phone_config(200, Kind::Phone), now);
    net.host.opened(now, refused, Caps::DATAGRAM);
    net.host.received(now, refused, &hello);
    net.pump_host();
    net.cut(0);
    net.phones[0].frozen = true;
    net.host.received(net.now, refused, &hello);
    net.run(3_000);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Arrived { .. })),
        max
    );
    let newcomer = net.add_phone(phone_config(201, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, newcomer);
}

/// One hello, held back past the tab that came after: whatever the order,
/// one tab plays and the other waits.
#[test]
fn a_hello_held_back_leaves_one_tab_playing() {
    let mut net = Net::new(47, Faults::default());
    net.hold_next(2_000);
    let first = net.add_phone(phone_config(54, Kind::Phone), Caps::DATAGRAM);
    net.run(10);
    let second = net.add_phone(phone_config(54, Kind::Phone), Caps::DATAGRAM);
    net.run(5_000);
    let mut statuses = [
        net.phones[first].client.status(),
        net.phones[second].client.status(),
    ];
    statuses.sort_by_key(|status| *status == Status::Replaced);
    assert_eq!(statuses, [Status::Connected, Status::Replaced]);
}

fn states_from(net: &Net, device: DeviceId) -> Vec<Up> {
    net.happened
        .iter()
        .filter_map(|(_, event)| {
            if let DeviceEvent::State {
                device: from,
                state,
            } = event
                && *from == device
            {
                Some(state.clone())
            } else {
                None
            }
        })
        .collect()
}

/// C4, C10: a reloaded page is a new client that numbers from the start. The
/// host keeps the old page's numbers, and used to ignore and acknowledge what
/// the new one sent: a stick dead until it passed them.
#[test]
fn a_reloaded_page_is_heard_at_once() {
    let mut net = Net::new(32, Faults::default());
    let page = net.add_phone(phone_config(36, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, page);
    for n in 0..50 {
        net.phones[page].client.set_state(&Up::Pad(n, 0)).unwrap();
        net.run(50);
    }
    let before = states_from(&net, device).len();
    net.cut(page);
    net.phones[page].client.quit();
    let reloaded = net.add_phone(phone_config(36, Kind::Phone), Caps::DATAGRAM);
    assert_eq!(joined(&mut net, reloaded), device);
    for n in 0..10 {
        net.phones[reloaded]
            .client
            .set_state(&Up::Pad(n, 1))
            .unwrap();
        net.run(100);
    }
    let want: Vec<Up> = (0..10).map(|n| Up::Pad(n, 1)).collect();
    assert_eq!(states_from(&net, device)[before..], want);
}

/// A state already set when the welcome comes, on a page reloaded after many
/// states, gets through too.
#[test]
fn a_state_set_before_the_welcome_of_a_reloaded_page_counts() {
    let mut net = Net::new(33, Faults::default());
    let page = net.add_phone(phone_config(37, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, page);
    for n in 0..5 {
        net.phones[page].client.set_state(&Up::Pad(n, 0)).unwrap();
        net.run(50);
    }
    net.cut(page);
    net.phones[page].client.quit();
    let reloaded = net.add_phone(phone_config(37, Kind::Phone), Caps::DATAGRAM);
    net.phones[reloaded]
        .client
        .set_state(&Up::Pad(9, 9))
        .unwrap();
    joined(&mut net, reloaded);
    net.run(500);
    assert_eq!(states_from(&net, device).last(), Some(&Up::Pad(9, 9)));
}

/// What the old page held does not stay held by a page that holds nothing:
/// a game drops an input by its age.
#[test]
fn a_reloaded_page_holds_nothing_until_it_says_so() {
    let mut net = Net::new(34, Faults::default());
    let page = net.add_phone(phone_config(38, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, page);
    net.phones[page].client.set_state(&Up::Pad(1, 0)).unwrap();
    net.run(500);
    net.cut(page);
    net.phones[page].client.quit();
    let reloaded = net.add_phone(phone_config(38, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, reloaded);
    let held = net.host.device(device).and_then(|info| info.held_at);
    net.run(2_000);
    assert_eq!(net.host.device(device).and_then(|info| info.held_at), held);
}

/// A client and a host joined by hand, to choose which packets of the client
/// reach the host.
struct Pipe {
    now: Ms,
    host: salon_core::Host<Up, Down>,
    client: salon_core::Client<Up, Down>,
    link: salon_core::LinkId,
}

impl Pipe {
    fn new() -> Pipe {
        let now = Ms(1_000);
        let mut pipe = Pipe {
            now,
            host: salon_core::Host::new(now, net::host_config(1, Vec::new())),
            client: salon_core::Client::new(now, phone_config(39, Kind::Phone)),
            link: salon_core::LinkId(1),
        };
        pipe.join();
        pipe
    }

    /// Everything the client has to send now.
    fn client_sends(&mut self) -> Vec<Vec<u8>> {
        use salon_core::ClientOutput;
        let mut sent = Vec::new();
        while let Some(output) = self.client.poll(self.now) {
            match output {
                ClientOutput::Connect => {
                    self.client.opened(self.now, Caps::DATAGRAM);
                    self.host.opened(self.now, self.link, Caps::DATAGRAM);
                }
                ClientOutput::Transmit(bytes) => sent.push(bytes),
                ClientOutput::Disconnect
                | ClientOutput::Status(_)
                | ClientOutput::Reset
                | ClientOutput::Event(_)
                | ClientOutput::State(_) => {}
            }
        }
        sent
    }

    /// What the host does now: its packets for this link, and what it heard.
    fn host_acts(&mut self) -> (Vec<Vec<u8>>, Vec<DeviceEvent<Up>>) {
        let (mut packets, mut heard) = (Vec::new(), Vec::new());
        while let Some(output) = self.host.poll(self.now) {
            match output {
                salon_core::Output::Transmit { link, bytes } if link == self.link => {
                    packets.push(bytes);
                }
                salon_core::Output::Device(event) => heard.push(event),
                salon_core::Output::Transmit { .. } | salon_core::Output::Close { .. } => {}
            }
        }
        (packets, heard)
    }

    fn join(&mut self) {
        for packet in self.client_sends() {
            self.host.received(self.now, self.link, &packet);
        }
        for packet in self.host_acts().0 {
            self.client.received(self.now, &packet);
        }
        assert_eq!(self.client.status(), Status::Connected);
    }

    /// The page reloads on a new link.
    fn reload(&mut self) {
        self.now = self.now.plus(10);
        self.client = salon_core::Client::new(self.now, phone_config(39, Kind::Phone));
        self.link = salon_core::LinkId(self.link.0.saturating_add(1));
        self.join();
    }

    /// One packet of the client reaches the host.
    fn deliver(&mut self, packet: &[u8]) -> Vec<DeviceEvent<Up>> {
        self.host.received(self.now, self.link, packet);
        self.host_acts().1
    }
}

/// A message half sent by the old page is not finished by fragments of the
/// new page's, which carry the same number: they would make one message of
/// two.
#[test]
fn a_half_sent_state_of_the_old_page_is_not_mixed_with_the_new_ones() {
    let mut pipe = Pipe::new();
    let (old, new) = (Up::Blob(vec![1; 3_000]), Up::Blob(vec![2; 3_000]));
    pipe.client.set_state(&old).unwrap();
    let packets = pipe.client_sends();
    assert!(packets.len() > 1, "the state must be cut");
    assert_eq!(pipe.deliver(&packets[0]), Vec::new());
    pipe.reload();
    pipe.client.set_state(&new).unwrap();
    let packets = pipe.client_sends();
    let mut heard = Vec::new();
    for packet in &packets[1..] {
        heard.extend(pipe.deliver(packet));
    }
    assert_eq!(heard, Vec::new(), "half of each page's state");
    heard.extend(pipe.deliver(&packets[0]));
    assert!(matches!(heard.as_slice(), [DeviceEvent::State { state, .. }] if *state == new));
}

/// The same for an event.
#[test]
fn a_half_sent_event_of_the_old_page_is_not_mixed_with_the_new_ones() {
    let mut pipe = Pipe::new();
    let (old, new) = (Up::Blob(vec![1; 3_000]), Up::Blob(vec![2; 3_000]));
    pipe.client.send(&old).unwrap();
    let packets = pipe.client_sends();
    assert!(packets.len() > 1, "the event must be cut");
    assert_eq!(pipe.deliver(&packets[0]), Vec::new());
    pipe.reload();
    pipe.client.send(&new).unwrap();
    let packets = pipe.client_sends();
    let mut heard = Vec::new();
    for packet in &packets[1..] {
        heard.extend(pipe.deliver(packet));
    }
    assert_eq!(heard, Vec::new(), "half of each page's event");
    heard.extend(pipe.deliver(&packets[0]));
    assert!(matches!(heard.as_slice(), [DeviceEvent::Event { event, .. }] if *event == new));
}

/// A packet with a hello as the last protocol wrote it, which this one does
/// not read: its `resume` had two fields.
fn hello_that_does_not_read(protocol: u16) -> Vec<u8> {
    use salon_core::Token;
    use serde::Serialize;

    #[derive(Serialize)]
    struct OldHello {
        protocol: u16,
        game: u32,
        kind: Kind,
        token: Token,
        name: Option<String>,
        key: Option<salon_core::TvKey>,
        resume: Option<(u32, DeviceId)>,
    }
    #[derive(Serialize)]
    enum OldControl {
        Hello(OldHello),
    }
    let packet = (
        (1_u32, 0_u64, None::<(u64, u32)>),
        Some(OldControl::Hello(OldHello {
            protocol,
            game: net::GAME,
            kind: Kind::Phone,
            token: Token([40; 16]),
            name: None,
            key: None,
            resume: Some((1, DeviceId(1))),
        })),
        (None::<(u32, u32)>, None::<u32>, None::<u32>),
        (Vec::<u8>::new(), Vec::<u8>::new()),
    );
    postcard::to_allocvec(&packet).expect("a packet encodes")
}

/// C8: a page left open through an upgrade of salon speaks the old protocol,
/// so its hello no longer reads. It is still told to reload, instead of
/// reconnecting for ever.
#[test]
fn a_page_of_another_protocol_is_told_so_even_when_its_hello_does_not_read() {
    use salon_core::{Client, ClientOutput, Host, LinkId, Output, PROTOCOL};

    let bytes = hello_that_does_not_read(PROTOCOL - 1);
    let now = Ms(1_000);
    let mut host: Host<Up, Down> = Host::new(now, net::host_config(1, Vec::new()));
    let link = LinkId(1);
    host.opened(now, link, Caps::DATAGRAM);
    host.received(now, link, &bytes);
    // The refusal reads the same in both protocols: the page understands it.
    let mut page: Client<Up, Down> = Client::new(now, phone_config(40, Kind::Phone));
    assert_eq!(page.poll(now), Some(ClientOutput::Connect));
    page.opened(now, Caps::DATAGRAM);
    while let Some(output) = host.poll(now) {
        if let Output::Transmit { bytes, .. } = output {
            page.received(now, &bytes);
        }
    }
    assert_eq!(page.status(), Status::Refused(Refusal::Version));
}

/// E1: only another number is another version. Broken bytes of this very
/// protocol are garbage, and telling their sender to reload would be a lie.
#[test]
fn a_broken_hello_of_this_protocol_is_garbage_and_not_another_version() {
    use salon_core::{Host, LinkId, Output, PROTOCOL};

    let bytes = hello_that_does_not_read(PROTOCOL);
    let now = Ms(1_000);
    let mut host: Host<Up, Down> = Host::new(now, net::host_config(1, Vec::new()));
    let link = LinkId(1);
    host.opened(now, link, Caps::DATAGRAM);
    host.received(now, link, &bytes);
    let mut told = Vec::new();
    while let Some(output) = host.poll(now) {
        told.push(output);
    }
    assert_eq!(told, Vec::<Output<Up>>::new());
}

/// Two tabs of one player, joined, the second one in the first one's place.
fn twins(net: &mut Net, token: u8) -> (usize, usize, DeviceId) {
    let first = net.add_phone(phone_config(token, Kind::Phone), Caps::DATAGRAM);
    let device = joined(net, first);
    let second = net.add_phone(phone_config(token, Kind::Phone), Caps::DATAGRAM);
    assert_eq!(joined(net, second), device);
    net.run(100);
    (first, second, device)
}

/// C5: "play here" after the other tab sent events. Both tabs count for the
/// same device, so the numbers of one are worth nothing to the other: they
/// were ignored and acknowledged, and `send` had returned `Ok`.
#[test]
fn play_here_keeps_counting_after_the_other_tab_sent_events() {
    let mut net = Net::new(35, Faults::default());
    let (first, second, device) = twins(&mut net, 41);
    for n in 0..3 {
        net.phones[second].client.send(&Up::Answer(n)).unwrap();
    }
    net.run(1_500);
    net.phones[first].client.send(&Up::Answer(3)).unwrap();
    net.phones[first].client.resume();
    assert!(net.run_until(3_000, |net| {
        net.phones[first].client.status() == Status::Connected
    }));
    net.phones[first].client.send(&Up::Answer(4)).unwrap();
    net.run(1_000);
    let want: Vec<Up> = (0..5).map(Up::Answer).collect();
    assert_eq!(net.events_from(device), want);
    assert_eq!(net.phones[second].client.status(), Status::Replaced);
}

/// The counters alone cannot tell: the other tab sent fewer events than this
/// one has waiting, so the host's number is not above this tab's, and what
/// waits was dropped as acknowledged.
#[test]
fn what_waits_in_a_tab_survives_the_other_one_sending_less() {
    let mut net = Net::new(36, Faults::default());
    let first = net.add_phone(phone_config(42, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, first);
    net.phones[first].frozen = true;
    for n in 0..3 {
        net.phones[first].client.send(&Up::Answer(n)).unwrap();
    }
    let second = net.add_phone(phone_config(42, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, second);
    net.phones[second].client.send(&Up::Answer(10)).unwrap();
    net.phones[second].client.send(&Up::Answer(11)).unwrap();
    net.run(1_500);
    // The first tab comes back from the background, knowing nothing of it.
    net.phones[first].frozen = false;
    assert!(net.run_until(10_000, |net| {
        net.phones[first].connects == 2 && net.phones[first].client.status() == Status::Connected
    }));
    net.run(1_000);
    let want: Vec<Up> = [10, 11, 0, 1, 2].map(Up::Answer).to_vec();
    assert_eq!(net.events_from(device), want);
}

/// The other way: an event that arrived but whose acknowledgement was lost
/// when the tab was replaced is not sent again by the tab that takes back.
#[test]
fn an_event_that_arrived_is_not_repeated_by_the_tab_that_takes_back() {
    let mut net = Net::new(37, Faults::default());
    let first = net.add_phone(phone_config(43, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, first);
    net.run(300);
    net.phones[first].client.send(&Up::Answer(1)).unwrap();
    assert!(net.run_until(100, |net| net.events_from(device).len() == 1));
    net.phones[first].frozen = true;
    let second = net.add_phone(phone_config(43, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, second);
    net.phones[second].client.send(&Up::Answer(2)).unwrap();
    net.run(1_500);
    net.phones[first].frozen = false;
    assert!(net.run_until(10_000, |net| {
        net.phones[first].connects == 2 && net.phones[first].client.status() == Status::Connected
    }));
    net.phones[first].client.send(&Up::Answer(3)).unwrap();
    net.run(1_000);
    assert_eq!(
        net.events_from(device),
        vec![Up::Answer(1), Up::Answer(2), Up::Answer(3)]
    );
}

/// A tab that plays again is heard like a new one, and what it held is
/// what the game holds now.
#[test]
fn play_here_is_heard_and_says_what_this_tab_holds() {
    let mut net = Net::new(38, Faults::default());
    let (first, second, device) = twins(&mut net, 44);
    for n in 0..5 {
        net.phones[second].client.set_state(&Up::Pad(n, 0)).unwrap();
        net.run(100);
    }
    net.phones[first].client.set_state(&Up::Pad(7, 7)).unwrap();
    net.phones[first].client.resume();
    assert!(net.run_until(3_000, |net| {
        net.phones[first].client.status() == Status::Connected
    }));
    net.run(500);
    assert_eq!(states_from(&net, device).last(), Some(&Up::Pad(7, 7)));
    net.phones[first].client.set_state(&Up::Pad(8, 8)).unwrap();
    net.run(500);
    assert_eq!(states_from(&net, device).last(), Some(&Up::Pad(8, 8)));
}

/// What the game hears from a tab whose event arrived, but not its
/// acknowledgement, then from `tabs` tabs that take over in turn and send
/// `each` events, and from the first tab again when it comes back.
fn heard_around_a_tab_away(tabs: u32, each: u32) -> Vec<Up> {
    let mut net = Net::new(39, Faults::default());
    let first = net.add_phone(phone_config(45, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, first);
    net.run(300);
    let room = "room for an event";
    net.phones[first].client.send(&Up::Answer(100)).expect(room);
    assert!(net.run_until(100, |net| net.events_from(device).len() == 1));
    net.phones[first].frozen = true;
    for tab in 0..tabs {
        let other = net.add_phone(phone_config(45, Kind::Phone), Caps::DATAGRAM);
        joined(&mut net, other);
        for n in 0..each {
            let event = Up::Answer(tab.saturating_mul(10).saturating_add(n));
            net.phones[other].client.send(&event).expect(room);
        }
        net.run(100);
    }
    net.phones[first].frozen = false;
    assert!(net.run_until(20_000, |net| {
        net.phones[first].connects >= 2 && net.phones[first].client.status() == Status::Connected
    }));
    net.run(1_000);
    net.events_from(device)
}

/// C5: the host remembers where the last eight tabs that sent something
/// stopped. The tab that comes back counts among them, as does the one it
/// replaces. Past eight, the tab may repeat what it had in flight.
#[test]
fn the_host_remembers_exactly_the_last_eight_tabs_that_sent_something() {
    let answers = |tabs: u32, numbers: Vec<u32>| {
        let sent = (0..tabs).map(|tab| Up::Answer(tab * 10));
        std::iter::once(Up::Answer(100))
            .chain(sent)
            .chain(numbers.into_iter().map(Up::Answer))
            .collect::<Vec<_>>()
    };
    assert_eq!(heard_around_a_tab_away(7, 1), answers(7, Vec::new()));
    assert_eq!(heard_around_a_tab_away(8, 1), answers(8, vec![100]));
}

/// A session that delivered nothing does not push out one that did: a tab
/// that only looked leaves no trace.
#[test]
fn tabs_that_send_nothing_do_not_count_among_the_eight() {
    assert_eq!(heard_around_a_tab_away(12, 0), vec![Up::Answer(100)]);
}

/// A tab away for more takeovers than the host remembers cannot be told what
/// arrived: it repeats what was in flight, and it loses nothing.
#[test]
fn a_tab_away_for_many_takeovers_loses_nothing() {
    let heard = heard_around_a_tab_away(12, 1);
    let want: Vec<Up> = std::iter::once(100)
        .chain((0..12).map(|tab| tab * 10))
        .chain([100])
        .map(Up::Answer)
        .collect();
    assert_eq!(heard, want);
}

/// What the player did while the host restarted is not void: `send` said
/// `Ok`, and the crate promises delivery across reconnections. The new run
/// hears of the device first.
#[test]
fn what_waits_goes_to_the_host_that_restarted() {
    let mut net = Net::new(40, Faults::default());
    let phone = net.add_phone(phone_config(46, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.run(500);
    let known = net.host.known();
    net.restart_host(2, known);
    let before = net.happened.len();
    net.phones[phone].client.send(&Up::Answer(7)).unwrap();
    assert!(net.run_until(15_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    net.run(1_000);
    assert_eq!(net.events_from(device), vec![Up::Answer(7)]);
    assert!(matches!(
        net.happened[before..].first(),
        Some((_, DeviceEvent::Arrived { device: d, .. })) if *d == device
    ));
    // The numbers restarted with the host, and go on from there.
    net.phones[phone].client.send(&Up::Answer(8)).unwrap();
    net.run(500);
    assert_eq!(net.events_from(device), vec![Up::Answer(7), Up::Answer(8)]);
}

/// The same when the host remembers nothing: a new device, for the game to
/// judge.
#[test]
fn what_waits_goes_to_a_host_without_memory_under_the_new_identity() {
    let mut net = Net::new(41, Faults::default());
    let phone = net.add_phone(phone_config(47, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.run(500);
    net.restart_host(2, Vec::new());
    net.phones[phone].client.send(&Up::Answer(7)).unwrap();
    assert!(net.run_until(15_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    net.run(1_000);
    let device = device_of(&net, phone);
    assert_eq!(net.events_from(device), vec![Up::Answer(7)]);
}

/// An event in flight when the host died may have reached it: it goes again,
/// since the new run knows nothing of it. Each run hears it once.
#[test]
fn an_event_in_flight_when_the_host_dies_reaches_each_run_once() {
    let mut net = Net::new(42, Faults::default());
    let phone = net.add_phone(phone_config(48, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.run(500);
    net.phones[phone].client.send(&Up::Answer(1)).unwrap();
    assert!(net.run_until(100, |net| net.events_from(device).len() == 1));
    // Its acknowledgement never comes back.
    net.phones[phone].frozen = true;
    let known = net.host.known();
    net.restart_host(2, known);
    net.run(50);
    net.phones[phone].frozen = false;
    assert!(net.run_until(15_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    net.run(1_000);
    assert_eq!(
        net.events_from(device),
        vec![Up::Answer(1), Up::Answer(1)],
        "once per run"
    );
}

/// A client with its attempt started at 0, in the budget of WebRTC then
/// WebSocket.
fn attempting() -> salon_core::Client<Up, Down> {
    use salon_core::{Client, ClientOutput};
    let mut client: Client<Up, Down> = Client::new(Ms(0), phone_config(49, Kind::Phone));
    assert_eq!(client.poll(Ms(0)), Some(ClientOutput::Connect));
    client.connecting_for(Ms(0), 20_000);
    client
}

/// C3: the page froze during an attempt, which may never end (Phoenix
/// 2575a6b). Waking up abandons it instead of waiting for its 20 s.
#[test]
fn waking_up_abandons_an_attempt_from_before_the_sleep() {
    use salon_core::ClientOutput;
    let mut client = attempting();
    let woke = Ms(8_000);
    client.wake(woke);
    assert_eq!(client.poll(woke), Some(ClientOutput::Disconnect));
    assert_eq!(client.next_deadline(), Some(woke));
    assert_eq!(client.poll(woke), Some(ClientOutput::Connect));
}

/// A page comes back to the foreground just as it opens: an attempt this
/// young is not stuck, and `pageshow` comes with every load.
#[test]
fn waking_up_leaves_a_young_attempt_alone() {
    let mut client = attempting();
    let woke = Ms(Timing::default().silent - 1);
    client.wake(woke);
    assert_eq!(client.poll(woke), None);
    assert_eq!(client.next_deadline(), Some(Ms(20_000)));
}

/// An attempt opened offline may never end on mobile Safari (centrifuge-js):
/// the network coming back abandons it, however young.
#[test]
fn the_network_coming_back_abandons_any_attempt() {
    use salon_core::ClientOutput;
    let mut client = attempting();
    let back = Ms(1);
    client.network_back(back);
    assert_eq!(client.poll(back), Some(ClientOutput::Disconnect));
    assert_eq!(client.poll(back), Some(ClientOutput::Connect));
}

/// The network coming back says nothing of a link that is alive.
#[test]
fn the_network_coming_back_leaves_a_live_link_alone() {
    let mut net = Net::new(43, Faults::default());
    let phone = net.add_phone(phone_config(50, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.run(1_000);
    net.phones[phone].client.network_back(net.now);
    net.run(1_000);
    assert_eq!(net.phones[phone].connects, 1);
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
}

/// A dead link is dropped as by a wake, and a planned attempt is brought
/// forward.
#[test]
fn the_network_coming_back_retries_at_once() {
    let mut net = Net::new(44, Faults::default());
    let phone = net.add_phone(phone_config(51, Kind::Phone), Caps::DATAGRAM);
    joined(&mut net, phone);
    net.phones[phone].frozen = true;
    net.run(3_000);
    net.phones[phone].frozen = false;
    net.phones[phone].client.network_back(net.now);
    assert!(net.run_until(500, |net| net.phones[phone].connects == 2
        && net.phones[phone].client.status() == Status::Connected));
}

const WALL: Ms = Ms(1_800_000_000_000);

/// The clock of a page may stop while the phone sleeps. The silence must
/// show all the same, at the wake, instead of up to 6 s later.
#[test]
fn a_clock_stopped_by_sleep_does_not_hide_the_silence() {
    use salon_core::{ClientOutput, SleepClock};
    let mut pipe = Pipe::new();
    pipe.client_sends();
    let mut clock = SleepClock::default();
    let start = clock.read(pipe.now, WALL);
    assert_eq!(start, pipe.now);
    // A minute asleep: the wall clock went on, the page's clock did not.
    let woke = clock.read(pipe.now.plus(20), WALL.plus(60_020));
    pipe.client.wake(woke);
    assert_eq!(pipe.client.poll(woke), Some(ClientOutput::Disconnect));
}

#[test]
fn a_clock_that_counts_sleep_is_left_as_it_is() {
    let mut clock = salon_core::SleepClock::default();
    clock.read(Ms(0), WALL);
    assert_eq!(clock.read(Ms(60_000), WALL.plus(60_000)), Ms(60_000));
    assert_eq!(clock.read(Ms(60_010), WALL.plus(60_010)), Ms(60_010));
}

/// A clock that stopped twice is ahead by both sleeps, and does not lose them
/// when the wall clock is set back.
#[test]
fn sleep_adds_up_and_time_never_goes_back() {
    let mut clock = salon_core::SleepClock::default();
    clock.read(Ms(0), WALL);
    assert_eq!(clock.read(Ms(0), WALL.plus(5_000)), Ms(5_000));
    assert_eq!(clock.read(Ms(0), WALL.plus(12_000)), Ms(12_000));
    assert_eq!(clock.read(Ms(10), WALL), Ms(12_010));
    assert_eq!(clock.read(Ms(20), WALL.plus(10)), Ms(12_020));
}

/// Drift between the two clocks, and small corrections of the wall clock, are
/// not sleep, however many readings they spread over.
#[test]
fn drift_is_not_sleep() {
    let mut clock = salon_core::SleepClock::default();
    clock.read(Ms(0), WALL);
    assert_eq!(clock.read(Ms(0), WALL.plus(999)), Ms(0));
    for n in 1..100 {
        assert_eq!(
            clock.read(Ms(n * 100), WALL.plus(999 + n * 105)),
            Ms(n * 100)
        );
    }
}

/// `Host::forget` drops a device, not a door: its token no longer names
/// anybody. Games count on that for a phone that never played, which comes
/// back as a new one.
#[test]
fn a_forgotten_device_comes_back_as_a_new_one() {
    let mut net = Net::new(45, Faults::default());
    let phone = net.add_phone(phone_config(52, Kind::Phone), Caps::DATAGRAM);
    let old = joined(&mut net, phone);
    net.phones[phone].frozen = true;
    net.run(7_000);
    net.host.forget(old);
    net.phones[phone].frozen = false;
    assert!(net.run_until(20_000, |net| net.phones[phone].resets == 1
        && net.phones[phone].client.status() == Status::Connected));
    assert!(device_of(&net, phone).0 > old.0);
    assert!(net.host.device(old).is_none());
}

/// The authority refuses a return by closing the room: `forget` then keeps
/// the player out, and the room can be left closed for as long as it likes.
#[test]
fn a_closed_room_keeps_a_forgotten_device_out() {
    let mut net = Net::new(46, Faults::default());
    let phone = net.add_phone(phone_config(53, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.host.close();
    net.host.forget(device);
    net.run(1_000);
    assert_eq!(
        net.phones[phone].client.status(),
        Status::Refused(Refusal::Removed)
    );
    net.phones[phone].client.resume();
    net.run(5_000);
    assert_eq!(
        net.phones[phone].client.status(),
        Status::Refused(Refusal::Closed)
    );
    assert_eq!(net.host.devices().count(), 0);
}

/// What it took to bring `events` of `size` bytes to a phone behind `narrow`.
struct Through {
    arrived: usize,
    seconds: f64,
    /// Bytes offered to the link for each useful one.
    cost: f64,
}

/// A phone behind `narrow` receives `events` events at once, in at most `ms`.
fn through(narrow: Narrow, caps: Caps, events: u32, size: usize, ms: u64) -> Through {
    let mut net = Net::new(5, Faults::default());
    net.narrow = Some(narrow);
    let phone = net.add_phone(phone_config(1, Kind::Phone), caps);
    let device = joined(&mut net, phone);
    net.run(1_000);
    net.offered = 0;
    net.dropped = 0;
    for n in 0..events {
        net.host
            .send(device, &Down::View(n, vec![0; size]))
            .expect("the queue has room");
    }
    let start = net.now;
    let want = usize::try_from(events).expect("a small number");
    net.run_until(ms, |net| net.phones[phone].events.len() == want);
    let sent: Vec<u32> = (0..events).collect();
    let got: Vec<u32> = net.phones[phone]
        .events
        .iter()
        .map(|event| match event {
            Down::View(n, _) | Down::Note(n) => *n,
        })
        .collect();
    assert_eq!(got, sent[..got.len()], "in order, and nothing twice");
    let cost = Ms(net.offered).as_f64() / Ms((size as u64) * u64::from(events)).as_f64();
    Through {
        arrived: got.len(),
        seconds: Ms(net.now.since(start)).as_f64() / 1_000.0,
        cost,
    }
}

/// T1. The buffer keeps the head of a burst and drops its tail: a resend that
/// starts from the same fragment would carry the same ones for ever, and what
/// follows in the queue would wait behind them.
#[test]
fn a_message_bigger_than_the_pipe_arrives() {
    for narrow in [
        Narrow::kilobits(1_000, 32 * 1024),
        Narrow::kilobits(5_000, 32 * 1024),
        Narrow::kilobits(20_000, 32 * 1024),
        Narrow::kilobits(300, Narrow::SCTP),
        Narrow::kilobits(100, Narrow::SCTP),
    ] {
        let one = through(narrow, Caps::DATAGRAM, 1, 60_000, 60_000);
        assert_eq!(one.arrived, 1, "{narrow:?}: {} s", one.seconds);
        let five = through(narrow, Caps::DATAGRAM, 5, 60_000, 120_000);
        assert_eq!(five.arrived, 5, "{narrow:?}: {} s", five.seconds);
    }
}

/// T3. A queue goes at the pace of its acknowledgements, not at once.
#[test]
fn a_full_queue_costs_less_than_twice_its_size() {
    let queue = through(
        Narrow::kilobits(1_000, Narrow::SCTP),
        Caps::DATAGRAM,
        20,
        8_000,
        60_000,
    );
    assert_eq!(queue.arrived, 20, "{} s", queue.seconds);
    assert!(queue.cost < 2.0, "{}", queue.cost);
}

/// T3. A dead link learns nothing from more of the same.
#[test]
fn a_silent_link_is_not_hammered() {
    let mut net = Net::new(5, Faults::default());
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.run(1_000);
    net.phones[phone].frozen = true;
    net.offered = 0;
    net.host
        .show(device, &Down::View(1, vec![0; 60_000]))
        .unwrap();
    net.run(Timing::default().absent - 100);
    assert!(net.offered < 8 * 60_000, "{}", net.offered);
}

/// T4. A WebSocket loses nothing: a copy of what is on its way only delays
/// what is fresh, and the state shown waits behind it.
#[test]
fn a_lossless_link_carries_no_copies() {
    let mut net = Net::new(5, Faults::default());
    net.narrow = Some(Narrow::kilobits(300, Narrow::OUTBOX));
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::STREAM);
    let device = joined(&mut net, phone);
    net.run(1_000);
    net.offered = 0;
    let (mut shown, every) = (0, 4_000);
    while shown < 15 {
        shown += 1;
        net.host
            .show(device, &Down::View(shown, vec![0; 60_000]))
            .unwrap();
        net.run(every);
    }
    net.run(every);
    let useful = 60_000 * u64::from(shown);
    assert!(net.offered < useful * 13 / 10, "{}", net.offered);
    assert!(net.deepest < 2 * 60_000, "{} bytes queued", net.deepest);
    assert_eq!(net.phones[phone].states.len(), 15);
    assert_eq!(net.phones[phone].connects, 1);
}

/// How long `events` events of `size` bytes take to reach a phone, on average
/// over a few runs.
fn delivery(faults: Faults, events: u32, size: usize) -> f64 {
    const RUNS: u32 = 5;
    let total: f64 = (0..u64::from(RUNS))
        .map(|seed| {
            let mut net = Net::new(seed, faults);
            let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
            let device = joined(&mut net, phone);
            net.run(500);
            for n in 0..events {
                net.host
                    .send(device, &Down::View(n, vec![0; size]))
                    .expect("the queue has room");
            }
            let start = net.now;
            let want = usize::try_from(events).expect("a small number");
            assert!(net.run_until(120_000, |net| net.phones[phone].events.len() == want));
            Ms(net.now.since(start)).as_f64() / 1_000.0
        })
        .sum();
    total / f64::from(RUNS)
}

/// T2. A hole holds up what follows, it does not cost it: what arrived past it
/// stays, and the hole is filled without waiting for the timer. A few
/// milliseconds of jitter on one burst were enough to send it all again.
#[test]
fn a_hole_in_the_queue_does_not_cost_what_follows() {
    let jitter = Faults {
        jitter: 5,
        ..Faults::default()
    };
    let unordered = delivery(jitter, 200, 900);
    assert!(unordered < 1.0, "{unordered} s");
    let lossy = Faults { loss: 10, ..jitter };
    let lost = delivery(lossy, 200, 900);
    assert!(lost < 1.5, "{lost} s");
}

/// T4. A WebSocket is lossless past the host's file to it, not before: when
/// that file is full, a packet is lost, and what comes back shows it. Here
/// one in five, more than any file would drop: the events all arrive.
#[test]
fn a_stream_that_drops_a_packet_recovers_it() {
    let mut net = Net::new(
        5,
        Faults {
            loss: 20,
            ..Faults::default()
        },
    );
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::STREAM);
    let device = joined(&mut net, phone);
    for n in 0..15 {
        net.host
            .send(device, &Down::View(n, vec![0; 60_000]))
            .unwrap();
        net.phones[phone].client.send(&Up::Answer(n)).unwrap();
    }
    assert!(
        net.run_until(20_000, |net| net.phones[phone].events.len() == 15
            && net.events_from(device).len() == 15)
    );
}

/// A hole in the proof that a stream dropped a packet. The phone's packets
/// lose their echo on the way, so nothing proves anything, the host keeps
/// hearing the phone, and a stream has no timer: what was dropped stays dropped.
/// The link is given up on, and what waited arrives on the next one, once.
#[test]
fn a_stream_with_a_hole_in_its_proof_comes_back() {
    let mut net = Net::new(5, Faults::default());
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::STREAM);
    let device = joined(&mut net, phone);
    net.tamper = Some(|bytes| {
        let mut packet = wire::decode(bytes);
        packet.echo = None;
        wire::encode(&packet)
    });
    net.faults.loss = 100;
    net.host.send(device, &Down::Note(1)).unwrap();
    net.stir();
    net.faults.loss = 0;
    let patience = 3 * Timing::default().absent;
    net.run(patience - 1_000);
    assert_eq!(net.phones[phone].events, []);
    assert_eq!(net.phones[phone].connects, 1, "not before its time");
    assert!(net.run_until(3_000, |net| net.phones[phone].events.len() == 1));
    assert_eq!(net.phones[phone].events, [Down::Note(1)]);
    assert_eq!(net.phones[phone].connects, 2);
}

/// A stream may be slow, and nobody gives up on it for that: well under what
/// T4 passes, with a queue of big states that takes a minute to go, no link
/// goes down and no copy leaves.
#[test]
fn a_slow_stream_is_not_taken_for_a_mute_one() {
    for kilobits in [300, 64, 32] {
        let mut net = Net::new(5, Faults::default());
        net.narrow = Some(Narrow::kilobits(kilobits, Narrow::OUTBOX));
        let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::STREAM);
        let device = joined(&mut net, phone);
        net.offered = 0;
        for n in 0..12 {
            net.host
                .send(device, &Down::View(n, vec![0; 60_000]))
                .unwrap();
        }
        let useful = 12 * 60_000;
        assert!(
            net.run_until(240_000, |net| net.phones[phone].events.len() == 12),
            "{kilobits} kb/s: {}",
            net.phones[phone].events.len()
        );
        assert_eq!(net.phones[phone].connects, 1, "{kilobits} kb/s");
        assert_eq!(
            net.count(|event| matches!(event, DeviceEvent::Left { .. })),
            0,
            "{kilobits} kb/s"
        );
        assert!(
            net.offered < useful * 11 / 10,
            "{kilobits} kb/s: {}",
            net.offered
        );
    }
}

/// T5. A state shown more often than it can arrive: each one used to wipe out
/// what was on its way, so none ever arrived. The one on its way finishes,
/// and the latest takes its place.
#[test]
fn a_big_state_refreshed_often_still_arrives() {
    for size in [30_000, 60_000] {
        let mut net = Net::new(
            3,
            Faults {
                loss: 10,
                jitter: 5,
                ..Faults::default()
            },
        );
        let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
        let device = joined(&mut net, phone);
        let (mut shown, every) = (0, 100);
        while shown < 300 {
            shown += 1;
            net.host
                .show(device, &Down::View(shown, vec![0; size]))
                .unwrap();
            net.run(every);
        }
        let seen = net.phones[phone].states.len();
        assert!(seen >= 60, "{size} bytes: {seen} of {shown}");
        net.run(3_000);
        assert_eq!(
            net.phones[phone].states.last(),
            Some(&Down::View(shown, vec![0; size])),
            "the latest arrives"
        );
    }
}

/// A room whose host has these limits, and a phone that keeps its own.
fn limited(limits: Limits, phone_limits: Limits) -> (Net, usize, DeviceId) {
    let mut net = Net::new(1, Faults::default());
    let config = HostConfig {
        limits,
        ..host_config(1, Vec::new())
    };
    net.host = Host::new(Ms(1_000), config);
    let config = ClientConfig {
        limits: phone_limits,
        ..phone_config(1, Kind::Phone)
    };
    let phone = net.add_phone(config, Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    (net, phone, device)
}

/// T8. Past 1,024 fragments no receiver takes a message: a limit that says
/// more is cut down when it is given, and `send` says so at once, instead of
/// blocking every event that follows.
#[test]
fn a_limit_past_what_fragments_carry_is_cut_down() {
    let limits = Limits {
        max_down: 2 * 1024 * 1024,
        max_up: 2 * 1024 * 1024,
        ..Limits::default()
    };
    let (mut net, phone, device) = limited(limits, limits);
    assert_eq!(
        net.host.send(device, &Down::View(1, vec![0; 1_100_000])),
        Err(SendError::TooBig)
    );
    assert_eq!(
        net.phones[phone].client.send(&Up::Blob(vec![0; 1_100_000])),
        Err(ClientSendError::TooBig)
    );
    net.host.send(device, &Down::Note(2)).unwrap();
    net.run(1_000);
    assert_eq!(net.phones[phone].events, vec![Down::Note(2)]);
}

/// T8, to the byte: 1,024 fragments of 940 bytes, tag, number and length of a
/// `Down::View` included.
#[test]
fn the_largest_message_is_the_last_one_a_receiver_takes() {
    let limits = Limits {
        max_down: 2 * 1024 * 1024,
        ..Limits::default()
    };
    let (mut net, phone, device) = limited(limits, limits);
    assert_eq!(
        net.host.send(device, &Down::View(1, vec![0; 962_556])),
        Err(SendError::TooBig)
    );
    net.host
        .send(device, &Down::View(1, vec![0; 962_555]))
        .unwrap();
    assert!(net.run_until(30_000, |net| net.phones[phone].events.len() == 1));
}

/// T9. `salon-web` keeps the default limits, and a host may have others: the
/// welcome says them. A page that took the default for the truth skipped what
/// it thought too big, or sent back what the host had to refuse.
#[test]
fn a_device_takes_the_limits_of_its_host() {
    let host = Limits {
        max_down: 256 * 1024,
        max_up: 1_000,
        ..Limits::default()
    };
    let (mut net, phone, device) = limited(host, Limits::default());
    net.host
        .send(device, &Down::View(1, vec![0; 100_000]))
        .unwrap();
    net.host
        .show(device, &Down::View(2, vec![0; 100_000]))
        .unwrap();
    net.host.send(device, &Down::Note(3)).unwrap();
    net.run(3_000);
    assert_eq!(net.phones[phone].events.len(), 2);
    assert_eq!(net.phones[phone].states.len(), 1);
    assert_eq!(
        net.phones[phone].client.send(&Up::Blob(vec![0; 2_000])),
        Err(ClientSendError::TooBig)
    );
}

/// T10. A goodbye may carry the last event of the page: it is read before the
/// link closes.
#[test]
fn an_event_sent_just_before_quitting_arrives() {
    let mut net = Net::new(1, Faults::default());
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].client.send(&Up::Answer(7)).unwrap();
    net.phones[phone].client.quit();
    net.run(2_000);
    assert_eq!(net.events_from(device), vec![Up::Answer(7)]);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::Left { why: Why::Quit, .. })),
        1
    );
}

/// T11. What waits for a device that does not answer is bounded in bytes, not
/// only in messages: the queue says `Full`, and one message is always let in.
#[test]
fn what_waits_for_a_device_is_bounded_in_bytes() {
    let mut net = Net::new(1, Faults::default());
    let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.phones[phone].frozen = true;
    let mut taken = 0;
    while net
        .host
        .send(device, &Down::View(taken, vec![0; 60_000]))
        .is_ok()
    {
        taken += 1;
        assert!(taken < 256, "only the count stopped it");
    }
    assert_eq!(taken, 17);
    assert_eq!(
        net.host.send(device, &Down::Note(0)),
        Ok(()),
        "a small event still fits"
    );

    let huge = Limits {
        max_down: 900 * 1024,
        ..Limits::default()
    };
    let (mut net, _, device) = limited(huge, huge);
    let big = || Down::View(0, vec![0; 800 * 1024]);
    assert_eq!(net.host.send(device, &big()), Ok(()));
    assert_eq!(net.host.send(device, &big()), Err(SendError::Full));
}

/// T12. `Timing` is public, and a game can give it the wrong way round: it is
/// put right when it is given, not found out by a panic on the room's thread.
#[test]
fn a_timing_the_wrong_way_round_does_not_stop_the_room() {
    let timing = Timing {
        resend_min: 2_000,
        resend_max: 1_000,
        ..Timing::default()
    };
    let mut net = Net::new(1, Faults::default());
    let config = HostConfig {
        timing,
        ..host_config(1, Vec::new())
    };
    net.host = Host::new(Ms(1_000), config);
    let config = ClientConfig {
        timing,
        ..phone_config(1, Kind::Phone)
    };
    let phone = net.add_phone(config, Caps::DATAGRAM);
    let device = joined(&mut net, phone);
    net.host.send(device, &Down::Note(1)).unwrap();
    net.run(3_000);
    assert_eq!(net.phones[phone].events, vec![Down::Note(1)]);
}
