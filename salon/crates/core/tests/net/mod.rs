//! A simulated network for salon: a host, phones, and packets that may be
//! lost, delayed, repeated and reordered, all from a seed. Time is virtual
//! and advances in steps; every run is reproducible.

#![expect(
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] functions: phone indices come from add_phone"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]

use std::collections::BTreeSet;

/// Which way a packet travels.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Toward {
    Host,
    Phone,
}

use salon_core::{
    Caps, Client, ClientConfig, ClientOutput, DeviceEvent, Epoch, Host, HostConfig, Kind, Known,
    Limits, LinkId, Ms, Output, Status, Timing, Token, TvKey,
};
use serde::{Deserialize, Serialize};

pub const GAME: u32 = 7;
pub const TV_KEY: TvKey = TvKey([9; 16]);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Up {
    Answer(u32),
    Pad(i8, i8),
    Blob(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Down {
    View(u32, Vec<u8>),
    Note(u32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    /// Percent of packets lost, each way.
    pub loss: u64,
    /// Percent of packets delivered twice.
    pub duplicate: u64,
    /// Extra delay, uniform from 0 to this, in ms (so packets reorder).
    pub jitter: u64,
}

pub struct Phone {
    pub client: Client<Up, Down>,
    pub link: Option<LinkId>,
    pub caps: Caps,
    /// Packets to and from this phone vanish, and closes are not seen: a
    /// locked iPhone, or Wi-Fi that died without a word.
    pub frozen: bool,
    pub events: Vec<Down>,
    pub states: Vec<Down>,
    pub statuses: Vec<Status>,
    pub resets: usize,
    pub connects: usize,
}

pub struct Net {
    pub now: Ms,
    pub host: Host<Up, Down>,
    pub phones: Vec<Phone>,
    pub happened: Vec<(Ms, DeviceEvent<Up>)>,
    pub faults: Faults,
    to_host: Vec<(Ms, LinkId, Vec<u8>)>,
    to_phone: Vec<(Ms, LinkId, Vec<u8>)>,
    closed_links: BTreeSet<LinkId>,
    /// Closes travel behind the data already sent, like a WebSocket's.
    closes_to_host: Vec<(Ms, LinkId)>,
    closes_to_phone: Vec<(Ms, LinkId)>,
    rng: u64,
    next_link: u64,
}

pub fn host_config(epoch: u32, known: Vec<Known>) -> HostConfig {
    HostConfig {
        game: GAME,
        epoch: Epoch(epoch),
        tv_key: TV_KEY,
        timing: Timing::default(),
        limits: Limits::default(),
        known,
    }
}

pub fn phone_config(token: u8, kind: Kind) -> ClientConfig {
    ClientConfig {
        game: GAME,
        kind,
        token: Token([token; 16]),
        name: Some(format!("Phone {token}")),
        tv_key: (kind == Kind::Tv).then_some(TV_KEY),
        seed: u64::from(token),
        timing: Timing::default(),
        limits: Limits::default(),
    }
}

impl Net {
    pub fn new(seed: u64, faults: Faults) -> Net {
        Net {
            now: Ms(1_000),
            host: Host::new(Ms(1_000), host_config(1, Vec::new())),
            phones: Vec::new(),
            happened: Vec::new(),
            faults,
            to_host: Vec::new(),
            to_phone: Vec::new(),
            closed_links: BTreeSet::new(),
            closes_to_host: Vec::new(),
            closes_to_phone: Vec::new(),
            rng: seed,
            next_link: 1,
        }
    }

    fn random(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn chance(&mut self, percent: u64) -> bool {
        percent > 0 && self.random() % 100 < percent
    }

    pub fn add_phone(&mut self, config: ClientConfig, caps: Caps) -> usize {
        let client = Client::new(self.now, config);
        self.phones.push(Phone {
            client,
            link: None,
            caps,
            frozen: false,
            events: Vec::new(),
            states: Vec::new(),
            statuses: Vec::new(),
            resets: 0,
            connects: 0,
        });
        self.phones.len() - 1
    }

    fn schedule(&mut self, toward: Toward, link: LinkId, bytes: &[u8]) {
        if toward == Toward::Host && self.closed_links.contains(&link) {
            return;
        }
        if self.chance(self.faults.loss) {
            return;
        }
        let copies = if self.chance(self.faults.duplicate) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let delay = 1 + if self.faults.jitter > 0 {
                self.random() % self.faults.jitter
            } else {
                0
            };
            let at = self.now.plus(delay);
            match toward {
                Toward::Host => self.to_host.push((at, link, bytes.to_vec())),
                Toward::Phone => self.to_phone.push((at, link, bytes.to_vec())),
            }
        }
    }

    fn phone_of(&self, link: LinkId) -> Option<usize> {
        self.phones
            .iter()
            .position(|phone| phone.link == Some(link))
    }

    /// Advances `ms` milliseconds, 5 ms at a time.
    pub fn run(&mut self, ms: u64) {
        let end = self.now.plus(ms);
        while self.now < end {
            self.now = self.now.plus(5).min(end);
            self.step();
        }
    }

    /// Advances until `done` holds, at most `ms`. Returns whether it held.
    pub fn run_until(&mut self, ms: u64, mut done: impl FnMut(&Net) -> bool) -> bool {
        let end = self.now.plus(ms);
        while self.now < end {
            if done(self) {
                return true;
            }
            self.now = self.now.plus(5).min(end);
            self.step();
        }
        done(self)
    }

    fn step(&mut self) {
        let now = self.now;
        // Deliver what is due, in arrival order.
        self.to_host.sort_by_key(|(at, _, _)| *at);
        let due = self
            .to_host
            .iter()
            .take_while(|(at, _, _)| *at <= now)
            .count();
        for (_, link, bytes) in self.to_host.drain(..due).collect::<Vec<_>>() {
            if !self.closed_links.contains(&link) {
                self.host.received(now, link, &bytes);
            }
        }
        self.to_phone.sort_by_key(|(at, _, _)| *at);
        let due = self
            .to_phone
            .iter()
            .take_while(|(at, _, _)| *at <= now)
            .count();
        for (_, link, bytes) in self.to_phone.drain(..due).collect::<Vec<_>>() {
            if let Some(index) = self.phone_of(link)
                && !self.phones[index].frozen
            {
                self.phones[index].client.received(now, &bytes);
            }
        }
        let due: Vec<(Ms, LinkId)> = self
            .closes_to_host
            .iter()
            .filter(|(at, _)| *at <= now)
            .copied()
            .collect();
        self.closes_to_host.retain(|(at, _)| *at > now);
        for (_, link) in due {
            self.host.closed(now, link);
        }
        let due: Vec<(Ms, LinkId)> = self
            .closes_to_phone
            .iter()
            .filter(|(at, _)| *at <= now)
            .copied()
            .collect();
        self.closes_to_phone.retain(|(at, _)| *at > now);
        for (_, link) in due {
            if let Some(index) = self.phone_of(link) {
                self.phones[index].link = None;
                if !self.phones[index].frozen {
                    self.phones[index].client.closed(now);
                }
            }
        }
        self.pump_host();
        for index in 0..self.phones.len() {
            self.pump_phone(index);
        }
        self.pump_host();
    }

    fn behind_data(&self) -> Ms {
        self.now.plus(self.faults.jitter + 2)
    }

    pub fn pump_host(&mut self) {
        let now = self.now;
        while let Some(output) = self.host.poll(now) {
            match output {
                Output::Transmit { link, bytes } => {
                    if let Some(index) = self.phone_of(link)
                        && !self.phones[index].frozen
                    {
                        self.schedule(Toward::Phone, link, &bytes);
                    }
                }
                Output::Close { link } => {
                    self.closed_links.insert(link);
                    let at = self.behind_data();
                    self.closes_to_phone.push((at, link));
                }
                Output::Device(event) => self.happened.push((now, event)),
            }
        }
    }

    fn pump_phone(&mut self, index: usize) {
        let now = self.now;
        while let Some(output) = self.phones[index].client.poll(now) {
            match output {
                ClientOutput::Connect => {
                    let link = LinkId(self.next_link);
                    self.next_link += 1;
                    let phone = &mut self.phones[index];
                    phone.connects += 1;
                    if phone.frozen {
                        // No network: the attempt fails after a while.
                        phone.link = Some(link);
                        continue;
                    }
                    phone.link = Some(link);
                    let caps = phone.caps;
                    phone.client.opened(now, caps);
                    self.host.opened(now, link, caps);
                }
                ClientOutput::Transmit(bytes) => {
                    if let Some(link) = self.phones[index].link
                        && !self.phones[index].frozen
                    {
                        self.schedule(Toward::Host, link, &bytes);
                    }
                }
                ClientOutput::Disconnect => {
                    if let Some(link) = self.phones[index].link.take()
                        && !self.phones[index].frozen
                    {
                        let at = self.behind_data();
                        self.closes_to_host.push((at, link));
                    }
                }
                ClientOutput::Status(status) => self.phones[index].statuses.push(status),
                ClientOutput::Reset => self.phones[index].resets += 1,
                ClientOutput::Event(event) => self.phones[index].events.push(event),
                ClientOutput::State(state) => self.phones[index].states.push(state),
            }
        }
    }

    /// Cuts a phone's transport cleanly: both sides see it close.
    pub fn cut(&mut self, index: usize) {
        if let Some(link) = self.phones[index].link.take() {
            self.closed_links.insert(link);
            self.host.closed(self.now, link);
            self.phones[index].client.closed(self.now);
        }
    }

    /// Replaces the host by a new run, as after a crash: links are gone
    /// without a word, and the new host knows `known`.
    pub fn restart_host(&mut self, epoch: u32, known: Vec<Known>) {
        self.host = Host::new(self.now, host_config(epoch, known));
        self.to_host.clear();
        for phone in &mut self.phones {
            if let Some(link) = phone.link {
                self.closed_links.insert(link);
            }
        }
    }

    pub fn events_from(&self, device: salon_core::DeviceId) -> Vec<Up> {
        self.happened
            .iter()
            .filter_map(|(_, event)| {
                if let DeviceEvent::Event {
                    device: from,
                    event,
                    ..
                } = event
                    && *from == device
                {
                    Some(event.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn count(&self, pick: impl Fn(&DeviceEvent<Up>) -> bool) -> usize {
        self.happened
            .iter()
            .filter(|(_, event)| pick(event))
            .count()
    }
}
