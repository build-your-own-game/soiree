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

use std::collections::{BTreeMap, BTreeSet};

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

/// What a link can carry, each way: past `rate`, packets wait in a buffer of
/// `buffer` bytes, and past that they are dropped from the tail. Picked to look
/// like what is in front of salon, never measured on a real network:
/// [`Narrow::SCTP`] has str0m's buffer, [`Narrow::OUTBOX`] the host's file of
/// packets to a WebSocket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Narrow {
    /// Bytes a second.
    pub rate: u64,
    pub buffer: usize,
}

impl Narrow {
    /// What str0m keeps of a data channel before it refuses a write.
    pub const SCTP: usize = 128 * 1024;
    /// What the host queues for a WebSocket before it loses a packet:
    /// `OUTBOX` packets of `Caps::STREAM`.
    pub const OUTBOX: usize = 256 * 16 * 1024;

    pub const fn kilobits(kilobits: u64, buffer: usize) -> Narrow {
        Narrow {
            rate: kilobits * 1000 / 8,
            buffer,
        }
    }
}

pub struct Phone {
    pub client: Client<Up, Down>,
    pub link: Option<LinkId>,
    pub caps: Caps,
    /// What the current link was opened with: `caps` is for the next one.
    on: Caps,
    /// Packets to and from this phone vanish, and closes are not seen: a
    /// locked iPhone, or Wi-Fi that died without a word.
    pub frozen: bool,
    /// The page runs nothing at all, like a tab the browser froze: no timer,
    /// no call. What reaches it stays in its socket until it thaws, closes
    /// included.
    pub asleep: bool,
    pub events: Vec<Down>,
    pub states: Vec<Down>,
    pub statuses: Vec<Status>,
    pub resets: usize,
    pub connects: usize,
    /// The next packet of this phone that survives comes this much later, and
    /// what follows it on that link waits behind it, as in a TCP stream.
    pub held: u64,
    stuck: Option<(LinkId, Ms)>,
    /// Something reached it since its last poll.
    stirred: bool,
}

/// What can happen to the host's process, past its own code.
#[derive(Clone, Copy, Debug, Default)]
pub struct Process {
    /// It stands still: what reaches it waits in its sockets.
    pub asleep: bool,
    /// There is none: a new transport is refused at once.
    pub gone: bool,
}

pub struct Net {
    pub now: Ms,
    pub host: Host<Up, Down>,
    pub phones: Vec<Phone>,
    pub happened: Vec<(Ms, DeviceEvent<Up>)>,
    pub faults: Faults,
    /// Every link is this narrow, and stays so when the faults heal.
    pub narrow: Option<Narrow>,
    /// Bytes the host offered its links, and those their buffers dropped.
    pub offered: u64,
    pub dropped: u64,
    /// Bytes waiting in a host's buffer, at the most.
    pub deepest: usize,
    /// Polls only on news and at `next_deadline`, as the shells do, instead
    /// of at every step: a deadline that comes too late then shows.
    pub lazy: bool,
    /// Something reached the host since its last poll.
    stirred: bool,
    pub process: Process,
    to_host: Vec<(Ms, LinkId, Vec<u8>)>,
    to_phone: Vec<(Ms, LinkId, Vec<u8>)>,
    closed_links: BTreeSet<LinkId>,
    /// Closes travel behind the data already sent, like a WebSocket's.
    closes_to_host: Vec<(Ms, LinkId)>,
    closes_to_phone: Vec<(Ms, LinkId)>,
    rng: u64,
    next_link: u64,
    /// The next packet, whichever way, comes this much later.
    held: u64,
    /// Each way of each link, in microseconds: when its buffer will be empty.
    busy: BTreeMap<(LinkId, bool), u64>,
    /// Each way of each stream, when its last packet arrives.
    latest: BTreeMap<(LinkId, bool), Ms>,
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

/// The closes that are due, and leave the list. A close never passes the data
/// sent on its link before it: TCP hands over what it holds, then the FIN.
fn due_closes(
    closes: &mut Vec<(Ms, LinkId)>,
    now: Ms,
    in_flight: &[(Ms, LinkId, Vec<u8>)],
) -> Vec<(Ms, LinkId)> {
    let blocked = |link: &LinkId| in_flight.iter().any(|(_, sent_on, _)| sent_on == link);
    let (due, later) = std::mem::take(closes)
        .into_iter()
        .partition(|(at, link)| *at <= now && !blocked(link));
    *closes = later;
    due
}

impl Net {
    pub fn new(seed: u64, faults: Faults) -> Net {
        Net {
            now: Ms(1_000),
            host: Host::new(Ms(1_000), host_config(1, Vec::new())),
            phones: Vec::new(),
            happened: Vec::new(),
            faults,
            narrow: None,
            offered: 0,
            dropped: 0,
            deepest: 0,
            lazy: false,
            stirred: true,
            process: Process::default(),
            to_host: Vec::new(),
            to_phone: Vec::new(),
            closed_links: BTreeSet::new(),
            closes_to_host: Vec::new(),
            closes_to_phone: Vec::new(),
            rng: seed,
            next_link: 1,
            held: 0,
            busy: BTreeMap::new(),
            latest: BTreeMap::new(),
        }
    }

    /// Holds back the next packet that survives, whichever way it goes, and
    /// delivers it `ms` later: one chosen packet, not a jitter on all of them.
    pub fn hold_next(&mut self, ms: u64) {
        self.held = ms;
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
            on: caps,
            frozen: false,
            asleep: false,
            events: Vec::new(),
            states: Vec::new(),
            statuses: Vec::new(),
            resets: 0,
            connects: 0,
            held: 0,
            stuck: None,
            stirred: true,
        });
        self.phones.len() - 1
    }

    /// The packet enters the link's buffer: when its last byte leaves, or
    /// `None` if there is no room for it.
    fn through_buffer(&mut self, toward: Toward, link: LinkId, len: usize) -> Option<Ms> {
        let Some(narrow) = self.narrow else {
            return Some(self.now);
        };
        // In front of a stream stands the file of the host, whatever the link.
        let buffer = if self.stream(link) {
            Narrow::OUTBOX
        } else {
            narrow.buffer
        };
        let now = self.now.0 * 1_000;
        let busy = self
            .busy
            .entry((link, toward == Toward::Host))
            .or_insert(now);
        let start = now.max(*busy);
        let waiting =
            usize::try_from((start - now) * narrow.rate / 1_000_000).unwrap_or(usize::MAX);
        // A buffer takes a packet whatever its size once it is empty: a packet
        // bigger than the buffer is not a packet that can never pass.
        if waiting > 0 && waiting + len > buffer {
            if toward == Toward::Phone {
                self.dropped += len as u64;
            }
            return None;
        }
        if toward == Toward::Phone {
            self.deepest = self.deepest.max(waiting + len);
        }
        *busy = start + len as u64 * 1_000_000 / narrow.rate;
        Some(Ms(*busy / 1_000))
    }

    fn stream(&self, link: LinkId) -> bool {
        self.phone_of(link)
            .is_some_and(|index| self.phones[index].on.max_packet > Caps::DATAGRAM.max_packet)
    }

    /// A stream hands packets over in order, however late one of them is.
    fn in_order(&mut self, toward: Toward, link: LinkId, at: Ms) -> Ms {
        if !self.stream(link) {
            return at;
        }
        let latest = self
            .latest
            .entry((link, toward == Toward::Host))
            .or_insert(at);
        *latest = at.max(*latest);
        *latest
    }

    fn schedule(&mut self, toward: Toward, link: LinkId, bytes: &[u8]) {
        if toward == Toward::Host && self.closed_links.contains(&link) {
            return;
        }
        let Some(left) = self.through_buffer(toward, link, bytes.len()) else {
            return;
        };
        if self.chance(self.faults.loss) {
            return;
        }
        let copies = if self.chance(self.faults.duplicate) {
            2
        } else {
            1
        };
        let held = std::mem::take(&mut self.held);
        let mut stuck = Ms(0);
        if toward == Toward::Host
            && let Some(index) = self.phone_of(link)
        {
            let phone = &mut self.phones[index];
            let held = std::mem::take(&mut phone.held);
            if held > 0 {
                phone.stuck = Some((link, self.now.plus(held)));
            }
            if let Some((on, until)) = phone.stuck
                && on == link
            {
                stuck = until;
            }
        }
        for _ in 0..copies {
            let delay = 1 + if self.faults.jitter > 0 {
                self.random() % self.faults.jitter
            } else {
                0
            };
            let at = left.plus(delay + held).max(stuck);
            let at = self.in_order(toward, link, at);
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

    /// A caller reached into the host or a client: everybody that runs looks
    /// at once, as a shell pumps within each call.
    pub fn stir(&mut self) {
        for index in 0..self.phones.len() {
            if !self.phones[index].asleep {
                self.pump_phone(index);
            }
        }
        if !self.process.asleep {
            self.pump_host();
        }
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
        let due = if self.process.asleep {
            0
        } else {
            self.to_host
                .iter()
                .take_while(|(at, _, _)| *at <= now)
                .count()
        };
        for (_, link, bytes) in self.to_host.drain(..due).collect::<Vec<_>>() {
            if !self.closed_links.contains(&link) {
                self.host.received(now, link, &bytes);
                self.stirred = true;
            }
        }
        self.to_phone.sort_by_key(|(at, _, _)| *at);
        for (at, link, bytes) in std::mem::take(&mut self.to_phone) {
            let phone = self.phone_of(link);
            if at > now || phone.is_some_and(|index| self.phones[index].asleep) {
                self.to_phone.push((at, link, bytes));
            } else if let Some(index) = phone
                && !self.phones[index].frozen
            {
                self.phones[index].client.received(now, &bytes);
                self.phones[index].stirred = true;
            }
        }
        if !self.process.asleep {
            for (_, link) in due_closes(&mut self.closes_to_host, now, &self.to_host) {
                self.host.closed(now, link);
                self.stirred = true;
            }
        }
        for (at, link) in due_closes(&mut self.closes_to_phone, now, &self.to_phone) {
            if let Some(index) = self.phone_of(link) {
                if self.phones[index].asleep {
                    self.closes_to_phone.push((at, link));
                    continue;
                }
                self.phones[index].link = None;
                if !self.phones[index].frozen {
                    self.phones[index].client.closed(now);
                    self.phones[index].stirred = true;
                }
            }
        }
        self.pump_host_if_due();
        for index in 0..self.phones.len() {
            let phone = &self.phones[index];
            let due = phone.client.next_deadline().is_some_and(|at| at <= now);
            if !phone.asleep && (!self.lazy || phone.stirred || due) {
                self.phones[index].stirred = false;
                self.pump_phone(index);
            }
        }
        self.pump_host_if_due();
    }

    fn pump_host_if_due(&mut self) {
        let due = self.host.next_deadline().is_some_and(|at| at <= self.now);
        if !self.process.asleep && (!self.lazy || self.stirred || due) {
            self.stirred = false;
            self.pump_host();
        }
    }

    fn behind_data(&self) -> Ms {
        self.now.plus(self.faults.jitter + 2)
    }

    pub fn pump_host(&mut self) {
        let now = self.now;
        while let Some(output) = self.host.poll(now) {
            match output {
                Output::Transmit { link, bytes } => {
                    self.offered += bytes.len() as u64;
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
        // A shell sleeps until the deadline: one that has come when nothing is
        // left to do turns on the spot, on the room's only thread.
        if let Some(at) = self.host.next_deadline() {
            assert!(at > now, "the host would spin: {at:?} at {now:?}");
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
                    if self.process.gone && !phone.frozen {
                        phone.client.closed(now);
                        continue;
                    }
                    if phone.frozen {
                        // No network: the attempt fails after a while.
                        phone.link = Some(link);
                        continue;
                    }
                    phone.link = Some(link);
                    let caps = phone.caps;
                    phone.on = caps;
                    phone.client.opened(now, caps);
                    self.host.opened(now, link, caps);
                    self.stirred = true;
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
        if let Some(at) = self.phones[index].client.next_deadline() {
            assert!(at > now, "a page would spin: {at:?} at {now:?}");
        }
    }

    /// Cuts a phone's transport cleanly: both sides see it close, and what it
    /// was carrying is lost.
    pub fn cut(&mut self, index: usize) {
        if let Some(link) = self.phones[index].link.take() {
            self.closed_links.insert(link);
            self.host.closed(self.now, link);
            self.phones[index].client.closed(self.now);
            self.stirred = true;
            self.phones[index].stirred = true;
        }
    }

    /// The page goes away and its browser closes the transport: like a
    /// WebSocket's, the close comes behind what the page already sent, however
    /// late that is. The host hears of it then.
    pub fn leave(&mut self, index: usize) {
        if let Some(link) = self.phones[index].link.take() {
            let at = self.behind_data();
            self.closes_to_host.push((at, link));
            self.phones[index].client.closed(self.now);
        }
        self.phones[index].client.quit();
    }

    /// Replaces the host by a new run, as after a crash: links are gone
    /// without a word, and the new host knows `known`.
    pub fn restart_host(&mut self, epoch: u32, known: Vec<Known>) {
        self.host = Host::new(self.now, host_config(epoch, known));
        self.stirred = true;
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
