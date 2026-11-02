//! Bugs found by reading what mutation testing pointed at, each reproduced
//! here first. Those fixed stay as regressions. An open one states what salon
//! promises and fails today, so it is ignored: an ignored test proves nothing,
//! and the fix removes its `#[ignore]`.
//! `cargo test -p salon-core --test found_bugs -- --ignored` shows them.

#![expect(
    clippy::expect_used,
    reason = "test helpers outside #[test] functions: a client without a hello stops the test"
)]

#[expect(
    dead_code,
    reason = "the simulated network is shared with tests/salon.rs, which uses all of it"
)]
mod net;

use net::{Down, Faults, Net, Up, host_config, phone_config};
use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, Host, Kind, LinkId, Ms, Name, Output,
    Refusal, Status, Token,
};

type Room = Host<Up, Down>;

/// The bytes of the first hello a client with this configuration sends.
fn hello_of(config: ClientConfig, now: Ms) -> Vec<u8> {
    let mut client: Client<Up, Down> = Client::new(now, config);
    loop {
        match client.poll(now).expect("a hello") {
            ClientOutput::Connect => client.opened(now, Caps::DATAGRAM),
            ClientOutput::Transmit(bytes) => return bytes,
            ClientOutput::Disconnect
            | ClientOutput::Status(_)
            | ClientOutput::Reset
            | ClientOutput::Event(_)
            | ClientOutput::State(_) => {}
        }
    }
}

fn phone_named(token: u8) -> ClientConfig {
    ClientConfig {
        token: Token([token; 16]),
        ..phone_config(token, Kind::Phone)
    }
}

fn wrong_game(token: u8) -> ClientConfig {
    ClientConfig {
        game: 99,
        ..phone_named(token)
    }
}

/// Polls for `ms`, 100 ms at a time, and returns the devices that left.
fn run(host: &mut Room, from: Ms, ms: u64) -> (Ms, u32) {
    let mut now = from;
    let mut left = 0_u32;
    for _ in 0..ms / 100 {
        now = now.plus(100);
        while let Some(output) = host.poll(now) {
            if matches!(output, Output::Device(DeviceEvent::Left { .. })) {
                left = left.saturating_add(1);
            }
        }
    }
    (now, left)
}

/// A hello on a link that was already refused used to be accepted, while the
/// link closed after its grace without detaching the device: the game was never
/// told, and the device stayed on a dead link for ever.
#[test]
fn a_hello_on_a_refused_link_is_not_heard() {
    let now = Ms(1_000);
    let mut host = Room::new(now, host_config(1, Vec::new()));
    let link = LinkId(5);
    host.opened(now, link, Caps::DATAGRAM);
    host.received(now, link, &hello_of(wrong_game(1), now));
    host.received(now, link, &hello_of(phone_named(1), now));
    let (_, left) = run(&mut host, now, 20_000);
    assert_eq!(left, 0);
    assert_eq!(host.devices().count(), 0);
}

/// The same, as an attack: each ghost kept a seat in `Limits::max_devices` for
/// ever, so 48 packets closed the room to everybody, against E3.
#[test]
fn ghosts_do_not_fill_the_room() {
    let now = Ms(1_000);
    let mut net = Net::new(1, Faults::default());
    for n in 0..24_u8 {
        let link = LinkId(1_000 + u64::from(n));
        net.host.opened(now, link, Caps::DATAGRAM);
        net.host
            .received(now, link, &hello_of(wrong_game(n + 1), now));
        net.host
            .received(now, link, &hello_of(phone_named(n + 1), now));
    }
    net.run(20_000);
    let phone = net.add_phone(phone_config(100, Kind::Phone), Caps::DATAGRAM);
    net.run(3_000);
    assert_eq!(
        net.phones[phone].client.status(),
        Status::Connected,
        "the room says {:?}",
        Status::Refused(Refusal::Full)
    );
}

/// The same root, without malice: a tab's hello, repeated and delayed, arrives
/// on its link after a second tab replaced it. The closing link took the
/// device back and the living tab was told it was replaced.
#[test]
fn a_late_hello_on_a_replaced_link_does_not_take_the_device_back() {
    let now = Ms(1_000);
    let (old, new) = (LinkId(5), LinkId(6));
    let hello = hello_of(phone_named(1), now);
    let mut host = Room::new(now, host_config(1, Vec::new()));
    host.opened(now, old, Caps::DATAGRAM);
    host.received(now, old, &hello);
    host.opened(now, new, Caps::DATAGRAM);
    host.received(now, new, &hello);
    host.received(now, old, &hello);
    let mut closed = Vec::new();
    let mut later = now;
    for _ in 0..30 {
        later = later.plus(100);
        host.received(later, new, &hello);
        while let Some(output) = host.poll(later) {
            if let Output::Close { link } = output {
                closed.push(link);
            }
        }
    }
    assert_eq!(closed, vec![old]);
}

/// A reloaded page is a new `Client` under the same token: it
/// used to number its states from 1, while the host's `in_state` stayed at the
/// old page's last number, ignored them and acknowledged them: the game did not
/// hear a phone's controls until the new page caught up.
#[test]
fn a_reloaded_page_is_heard_when_it_sets_its_state() {
    let mut net = Net::new(1, Faults::default());
    let page = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    assert!(net.run_until(3_000, |net| net.phones[page].client.status()
        == Status::Connected));
    let device = net.phones[page].client.device();
    for n in 0..3 {
        net.phones[page].client.set_state(&Up::Answer(n)).unwrap();
        net.run(300);
    }
    net.cut(page);
    net.phones[page].frozen = true;
    let reloaded = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
    assert!(
        net.run_until(8_000, |net| net.phones[reloaded].client.status()
            == Status::Connected)
    );
    assert_eq!(net.phones[reloaded].client.device(), device);
    net.phones[reloaded]
        .client
        .set_state(&Up::Answer(100))
        .unwrap();
    net.run(3_000);
    assert_eq!(
        net.count(|event| matches!(event, DeviceEvent::State { .. })),
        4
    );
}

/// A name that does not fit a packet: `encode_within` returned `None`, so no
/// hello ever left and the phone stayed on `Connecting`, without an error,
/// against the crate's "never a silent lobby". A name is cut anyway: the
/// client now cuts it first, on both transports.
#[test]
fn a_name_too_long_for_a_packet_does_not_hang_the_phone() {
    let mut net = Net::new(1, Faults::default());
    let phone = net.add_phone(
        ClientConfig {
            name: Some("x".repeat(2_000)),
            ..phone_config(1, Kind::Phone)
        },
        Caps::DATAGRAM,
    );
    assert!(
        net.run_until(30_000, |net| net.phones[phone].client.status()
            != Status::Connecting),
        "still connecting after 30 s"
    );
    assert_eq!(net.phones[phone].client.status(), Status::Connected);
    let device = net.phones[phone].client.device().expect("an identity");
    let name = net.host.device(device).and_then(|info| info.name);
    assert_eq!(name, Name::new(&"x".repeat(Name::MAX_CHARS)));
}

/// What the host keeps of a name does not depend on who cut it first.
#[test]
fn a_name_cut_by_the_client_is_the_one_the_host_would_keep() {
    let mut net = Net::new(1, Faults::default());
    let raw = format!("  Ada\u{7}{}  ", "é".repeat(100));
    let phone = net.add_phone(
        ClientConfig {
            name: Some(raw.clone()),
            ..phone_config(1, Kind::Phone)
        },
        Caps::DATAGRAM,
    );
    assert!(net.run_until(3_000, |net| net.phones[phone].client.status()
        == Status::Connected));
    let device = net.phones[phone].client.device().expect("an identity");
    assert_eq!(
        net.host.device(device).and_then(|info| info.name),
        Name::new(&raw)
    );
}

/// A name made of nothing a name can hold joins without a name, as a phone
/// without one.
#[test]
fn a_name_with_nothing_left_joins_without_a_name() {
    let mut net = Net::new(1, Faults::default());
    let phone = net.add_phone(
        ClientConfig {
            name: Some(" \u{7} ".repeat(1_000)),
            ..phone_config(1, Kind::Phone)
        },
        Caps::DATAGRAM,
    );
    assert!(net.run_until(3_000, |net| net.phones[phone].client.status()
        == Status::Connected));
    let device = net.phones[phone].client.device().expect("an identity");
    assert_eq!(net.host.device(device).and_then(|info| info.name), None);
}
