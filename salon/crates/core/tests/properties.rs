//! Stage 1 of `CERTIFICATION.md` for salon: random inputs and random
//! network chaos, invariants checked, failures shrunk to the smallest case.
//! `PROPTEST_CASES=5000 cargo test` searches further.

#[expect(
    dead_code,
    reason = "the simulated network is shared with tests/salon.rs, which uses all of it"
)]
mod net;

use net::{Down, Faults, Net, Up, phone_config};
use proptest::prelude::*;
use salon_core::{Caps, DeviceEvent, Kind, LinkId, Ms, Output, Status};

proptest! {
    /// E1: bytes from nowhere never crash the host, never reach the game,
    /// and a stranger that only sends garbage is dropped.
    #[test]
    fn garbage_never_reaches_the_game(packets in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..300), 0..40)) {
        let mut net = Net::new(1, Faults::default());
        let link = LinkId(999);
        net.host.opened(net.now, link, Caps::DATAGRAM);
        for (n, bytes) in packets.iter().enumerate() {
            net.host.received(Ms(1_000 + n as u64), link, bytes);
            while let Some(output) = net.host.poll(Ms(1_000 + n as u64)) {
                prop_assert!(!matches!(output, Output::Device(_)), "{output:?}");
            }
        }
        prop_assert_eq!(net.host.devices().count(), 0);
    }

    /// Garbage on a welcomed link is ignored too, and the device keeps
    /// working.
    #[test]
    fn garbage_from_a_phone_is_ignored(packets in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..200), 1..20)) {
        let mut net = Net::new(2, Faults::default());
        let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
        prop_assert!(net.run_until(3_000, |net| net.phones[phone].client.status() == Status::Connected));
        let device = net.phones[phone].client.device().unwrap();
        let link = net.phones[phone].link.unwrap();
        for bytes in &packets {
            net.host.received(net.now, link, bytes);
        }
        net.phones[phone].client.send(&Up::Answer(1)).unwrap();
        prop_assert!(net.run_until(3_000, |net| net.events_from(device) == vec![Up::Answer(1)]));
    }
}

#[derive(Clone, Debug)]
enum Step {
    /// A phone sends an event.
    Send(usize),
    /// The host sends an event to a phone.
    Notify(usize),
    /// The host changes what a phone shows.
    Show(usize),
    /// A phone's transport closes.
    Cut(usize),
    /// A phone goes silent, or comes back.
    Freeze(usize),
    Thaw(usize),
    /// Time passes.
    Wait(u64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (0..3usize).prop_map(Step::Send),
        3 => (0..3usize).prop_map(Step::Notify),
        2 => (0..3usize).prop_map(Step::Show),
        1 => (0..3usize).prop_map(Step::Cut),
        1 => (0..3usize).prop_map(Step::Freeze),
        1 => (0..3usize).prop_map(Step::Thaw),
        4 => (1..3_000u64).prop_map(Step::Wait),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Whatever the network does, every event arrives once and in order,
    /// every phone ends up connected as itself once the network is back,
    /// and a device never has two links.
    #[test]
    fn chaos_never_loses_or_doubles_an_event(
        seed in any::<u64>(),
        loss in 0..40u64,
        jitter in 0..120u64,
        steps in prop::collection::vec(step(), 1..60),
    ) {
        let mut net = Net::new(seed, Faults { loss, duplicate: 10, jitter });
        for token in 0..3u8 {
            net.add_phone(phone_config(token + 1, Kind::Phone), Caps::DATAGRAM);
        }
        net.run(500);
        let mut sent_up = [0u32; 3];
        let mut sent_down = [0u32; 3];
        let mut shown = [0u32; 3];
        for step in steps {
            match step {
                Step::Send(phone) => {
                    if net.phones[phone].client.send(&Up::Answer(sent_up[phone])).is_ok() {
                        sent_up[phone] += 1;
                    }
                }
                Step::Notify(phone) => {
                    if let Some(device) = net.phones[phone].client.device()
                        && net.host.send(device, &Down::Note(sent_down[phone])).is_ok()
                    {
                        sent_down[phone] += 1;
                    }
                }
                Step::Show(phone) => {
                    if let Some(device) = net.phones[phone].client.device() {
                        shown[phone] += 1;
                        net.host.show(device, &Down::View(shown[phone], vec![0; 3_000])).unwrap();
                    }
                }
                Step::Cut(phone) => net.cut(phone),
                Step::Freeze(phone) => net.phones[phone].frozen = true,
                Step::Thaw(phone) => net.phones[phone].frozen = false,
                Step::Wait(ms) => net.run(ms),
            }
        }
        // The network heals: everybody must come back and catch up.
        net.faults = Faults::default();
        for phone in &mut net.phones {
            phone.frozen = false;
        }
        let healed = net.run_until(30_000, |net| {
            net.phones.iter().all(|phone| phone.client.status() == Status::Connected)
        });
        prop_assert!(healed, "{:?}", net.phones.iter().map(|p| p.client.status()).collect::<Vec<_>>());
        net.run(3_000);

        for (index, phone) in net.phones.iter().enumerate() {
            let device = phone.client.device().unwrap();
            let want: Vec<Up> = (0..sent_up[index]).map(Up::Answer).collect();
            prop_assert_eq!(net.events_from(device), want, "phone {}", index);
            let want: Vec<Down> = (0..sent_down[index]).map(Down::Note).collect();
            prop_assert_eq!(&phone.events, &want, "phone {}", index);
            if shown[index] > 0 {
                prop_assert_eq!(
                    phone.states.last(),
                    Some(&Down::View(shown[index], vec![0; 3_000])),
                    "phone {}", index
                );
            }
            prop_assert_eq!(phone.resets, 0);
        }
        // Each device arrived once; a Back always follows a Left.
        let mut away = std::collections::BTreeMap::new();
        for (_, event) in &net.happened {
            match event {
                DeviceEvent::Arrived { device, .. } => {
                    prop_assert!(away.insert(*device, false).is_none(), "{device} arrived twice");
                }
                DeviceEvent::Left { device, .. } => {
                    prop_assert_eq!(away.insert(*device, true), Some(false), "{} left twice", device);
                }
                DeviceEvent::Back { device } => {
                    prop_assert_eq!(away.insert(*device, false), Some(true), "{} back without leaving", device);
                }
                _ => {}
            }
        }
    }
}
