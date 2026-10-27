//! Seeded in-process network around the real host driver and real clients.
//! No socket, sleep, wall clock or OS randomness participates in a run.

use crate::{
    Game,
    driver::{Driver, Effect, Input},
};
use salon_core::{Caps, Client, ClientConfig, ClientOutput, LinkId, Ms, Status};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    pub loss: u8,
    pub duplicate: u8,
    pub jitter: u64,
}

pub struct Phone<Up, Down> {
    pub client: Client<Up, Down>,
    pub link: Option<LinkId>,
    pub caps: Caps,
    pub frozen: bool,
    pub events: Vec<Down>,
    pub states: Vec<Down>,
    pub statuses: Vec<Status>,
    pub connects: u64,
}

#[derive(Clone, Copy)]
enum Toward {
    Host,
    Phone,
}
struct Packet {
    at: Ms,
    link: LinkId,
    toward: Toward,
    bytes: Vec<u8>,
}

pub struct Network<G: Game> {
    pub now: Ms,
    pub driver: Driver<G>,
    pub phones: Vec<Phone<G::Up, G::Down>>,
    pub faults: Faults,
    rng: u64,
    next_link: u64,
    packets: Vec<Packet>,
    pending: Vec<Input>,
}

impl<G: Game> Network<G>
where
    G::Up: Serialize,
    G::Down: DeserializeOwned,
{
    pub fn new(seed: u64, faults: Faults, driver: Driver<G>) -> Self {
        Self {
            now: driver.now(),
            driver,
            phones: Vec::new(),
            faults,
            rng: seed,
            next_link: 1,
            packets: Vec::new(),
            pending: Vec::new(),
        }
    }
    pub fn add_phone(&mut self, config: ClientConfig, caps: Caps) -> usize {
        let index = self.phones.len();
        self.phones.push(Phone {
            client: Client::new(self.now, config),
            link: None,
            caps,
            frozen: false,
            events: Vec::new(),
            states: Vec::new(),
            statuses: Vec::new(),
            connects: 0,
        });
        index
    }
    /// A visible transport cut: both ends hear it immediately.
    pub fn cut(&mut self, index: usize) {
        if let Some(phone) = self.phones.get_mut(index) {
            if let Some(link) = phone.link.take() {
                self.pending.push(Input::Closed { link });
            }
            phone.client.closed(self.now);
        }
    }
    pub fn freeze(&mut self, index: usize) {
        if let Some(phone) = self.phones.get_mut(index) {
            phone.frozen = true;
        }
    }
    pub fn wake(&mut self, index: usize) {
        if let Some(phone) = self.phones.get_mut(index) {
            phone.frozen = false;
            phone.client.wake(self.now);
        }
    }
    /// Advances directly to the next packet or client/host deadline. State
    /// cannot change between events, so invariants need checking only after
    /// these steps, not once per idle virtual millisecond.
    pub fn run(&mut self, milliseconds: u64) {
        self.run_checked(milliseconds, |_| {});
    }
    pub fn run_checked(&mut self, milliseconds: u64, mut check: impl FnMut(&Self)) {
        let end = self.now.plus(milliseconds);
        while self.now < end {
            self.now = self.next_event(end);
            self.step();
            check(self);
        }
    }
    pub fn run_until(&mut self, milliseconds: u64, mut done: impl FnMut(&Self) -> bool) -> bool {
        let end = self.now.plus(milliseconds);
        while self.now < end && !done(self) {
            self.now = self.next_event(end);
            self.step();
        }
        done(self)
    }
    fn next_event(&self, end: Ms) -> Ms {
        let packet = self.packets.iter().map(|packet| packet.at).min();
        let phone = self
            .phones
            .iter()
            .filter(|phone| !phone.frozen)
            .filter_map(|phone| phone.client.next_deadline())
            .min();
        [Some(end), Some(self.driver.next_deadline()), packet, phone]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(end)
            .max(self.now.plus(1))
            .min(end)
    }

    fn random(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn schedule(&mut self, toward: Toward, link: LinkId, bytes: &[u8]) {
        if self.random() % 100 < u64::from(self.faults.loss) {
            return;
        }
        let copies = if self.random() % 100 < u64::from(self.faults.duplicate) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            // A bound makes a deliberately impossible test configuration
            // behave like a saturated physical link, never unbounded memory.
            if self.packets.len() >= 16_384 {
                return;
            }
            let delay = self
                .random()
                .checked_rem(self.faults.jitter.saturating_add(1))
                .unwrap_or(0);
            self.packets.push(Packet {
                at: self.now.plus(delay.saturating_add(1)),
                toward,
                link,
                bytes: bytes.to_vec(),
            });
        }
    }
    fn effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Transmit { link, bytes } => {
                    if self
                        .phones
                        .iter()
                        .any(|phone| phone.link == Some(link) && !phone.frozen)
                    {
                        self.schedule(Toward::Phone, link, &bytes);
                    }
                }
                Effect::Close { link } => {
                    if let Some(phone) = self
                        .phones
                        .iter_mut()
                        .find(|phone| phone.link == Some(link))
                    {
                        phone.link = None;
                        if !phone.frozen {
                            phone.client.closed(self.now);
                        }
                    }
                }
            }
        }
    }
    fn step(&mut self) {
        self.packets.sort_by_key(|packet| packet.at);
        let count = self
            .packets
            .iter()
            .take_while(|packet| packet.at <= self.now)
            .count();
        for packet in self.packets.drain(..count).collect::<Vec<_>>() {
            let phone = self
                .phones
                .iter_mut()
                .find(|phone| phone.link == Some(packet.link) && !phone.frozen);
            if let Some(phone) = phone {
                match packet.toward {
                    Toward::Host => self.pending.push(Input::Received {
                        link: packet.link,
                        bytes: packet.bytes,
                    }),
                    Toward::Phone => phone.client.received(self.now, &packet.bytes),
                }
            }
        }
        let effects = self
            .driver
            .step(self.now, std::mem::take(&mut self.pending));
        self.effects(effects);
        for index in 0..self.phones.len() {
            while let Some(phone) = self.phones.get_mut(index).filter(|phone| !phone.frozen) {
                let Some(output) = phone.client.poll(self.now) else {
                    break;
                };
                match output {
                    ClientOutput::Connect => {
                        let link = LinkId(self.next_link);
                        self.next_link = self.next_link.saturating_add(1);
                        phone.connects = phone.connects.saturating_add(1);
                        phone.link = Some(link);
                        phone.client.opened(self.now, phone.caps);
                        self.pending.push(Input::Opened {
                            link,
                            caps: phone.caps,
                        });
                    }
                    ClientOutput::Transmit(bytes) => {
                        if let Some(link) = phone.link {
                            self.schedule(Toward::Host, link, &bytes);
                        }
                    }
                    ClientOutput::Disconnect => {
                        if let Some(link) = phone.link.take() {
                            self.pending.push(Input::Closed { link });
                        }
                    }
                    ClientOutput::Status(status) => phone.statuses.push(status),
                    ClientOutput::Reset => {
                        phone.states.clear();
                        phone.events.clear();
                    }
                    ClientOutput::Event(event) => {
                        if phone.events.len() >= 128 {
                            phone.events.remove(0);
                        }
                        phone.events.push(event);
                    }
                    ClientOutput::State(state) => {
                        if phone.states.len() >= 128 {
                            phone.states.remove(0);
                        }
                        phone.states.push(state);
                    }
                }
            }
        }
        let effects = self
            .driver
            .step(self.now, std::mem::take(&mut self.pending));
        self.effects(effects);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use salon_core::{
        DeviceEvent, DeviceId, Epoch, Host, HostConfig, Kind, Limits, Timing, Token, TvKey,
    };
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Counter {
        counts: BTreeMap<DeviceId, u32>,
        ticks: u64,
    }
    impl Game for Counter {
        type Up = u32;
        type Down = u32;
        fn on(&mut self, _: Ms, event: DeviceEvent<u32>, host: &mut Host<u32, u32>) {
            match event {
                DeviceEvent::Arrived { device, .. } => {
                    self.counts.entry(device).or_default();
                    let _: Result<(), _> = host.show(device, &0);
                }
                DeviceEvent::Event { device, event, .. } => {
                    let count = self.counts.entry(device).or_default();
                    *count = count.saturating_add(event);
                    let _: Result<(), _> = host.show(device, count);
                }
                DeviceEvent::Back { .. } | DeviceEvent::Left { .. } | DeviceEvent::State { .. } => {
                }
            }
        }
        fn tick(&mut self, _: Ms, _: &mut Host<u32, u32>) {
            self.ticks = self.ticks.saturating_add(1);
        }
    }
    fn network(seed: u64) -> Network<Counter> {
        let config = HostConfig {
            game: 1,
            epoch: Epoch(1),
            tv_key: TvKey([0; 16]),
            timing: Timing::default(),
            limits: Limits::default(),
            known: Vec::new(),
        };
        let driver = Driver::new(Ms(0), config, 17, Counter::default());
        let mut network = Network::new(
            seed,
            Faults {
                loss: 5,
                duplicate: 10,
                jitter: 20,
            },
            driver,
        );
        network.add_phone(
            ClientConfig {
                game: 1,
                kind: Kind::Phone,
                token: Token([1; 16]),
                name: None,
                tv_key: None,
                seed,
                timing: Timing::default(),
                limits: Limits::default(),
            },
            Caps::DATAGRAM,
        );
        network
    }
    #[test]
    fn event_driven_steps_match_a_one_millisecond_reference() {
        let mut fast = network(42);
        let mut reference = network(42);
        fast.run(2000);
        for _ in 0..2000 {
            reference.now = reference.now.plus(1);
            reference.step();
        }
        assert_eq!(fast.driver.game().counts, reference.driver.game().counts);
        assert_eq!(fast.driver.game().ticks, reference.driver.game().ticks);
        assert_eq!(
            fast.phones.first().unwrap().states,
            reference.phones.first().unwrap().states
        );
        assert_eq!(
            fast.phones.first().unwrap().client.device(),
            reference.phones.first().unwrap().client.device()
        );
        let mut steps = 0_u64;
        fast.run_checked(10_000, |_| steps = steps.saturating_add(1));
        assert!(steps < 2_000, "idle steps must be skipped: {steps}");
    }
    #[test]
    fn repeated_seed_replays_whole_game_network_and_timing() {
        for seed in 0..256 {
            let run = || {
                let mut net = network(seed);
                assert!(
                    net.run_until(2_000, |net| net
                        .phones
                        .first()
                        .is_some_and(|phone| phone.client.status() == Status::Connected)),
                    "join seed {seed}"
                );
                let before = net.phones.first().unwrap().client.device();
                net.phones.first_mut().unwrap().client.send(&7).unwrap();
                net.run(500);
                net.cut(0);
                net.run(2_500);
                let after = net.phones.first().unwrap().client.device();
                assert_eq!(before, after, "identity seed {seed}");
                assert_eq!(
                    net.driver.game().counts.values().copied().sum::<u32>(),
                    7,
                    "once seed {seed}"
                );
                (
                    net.driver.game().counts.clone(),
                    net.driver.game().ticks,
                    net.phones.first().unwrap().states.clone(),
                )
            };
            assert_eq!(run(), run(), "replay seed {seed}");
        }
    }
}
