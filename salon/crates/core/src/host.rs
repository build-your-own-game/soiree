//! The host side: one `Host` owns every device of the room.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use core::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::stream::{Bucket, Budget, InEvents, InState, Order, OutEvents, OutState, Peer, Window};
use crate::types::{
    Caps, DeviceId, Epoch, Kind, Limits, LinkId, Ms, Name, Presence, Refusal, Timing, Token, TvKey,
    Why,
};
use crate::wire::{self, Ack, Control, Hello, MAX_PACKET, PROTOCOL, Packet, Resume, Welcome};

/// Packets one link may send in one flush, to drain a large message.
const BURST: usize = 32;
/// A final message (refused, replaced, closing) goes this many times: one
/// lost packet must not leave a phone guessing.
const FINAL_COPIES: usize = 3;
/// Unreadable packets a link may send before it said who it is.
const GARBAGE_BEFORE_HELLO: u32 = 3;
/// After a final message, the link stays open this long so the copies can
/// arrive before it closes (Quake 3 keeps such a "zombie" 2 s).
const FINAL_GRACE: u64 = 1_000;
/// Sessions a device remembers the end of, so that a tab that comes back
/// after being replaced is told what arrived. Past that it may repeat what
/// it had in flight, never lose it. A session that delivered nothing, like a
/// welcome that got lost, does not count.
const PAST_SESSIONS: usize = 8;

pub struct HostConfig {
    /// The game's version: a phone with another one is refused.
    pub game: u32,
    /// Differs from every run a device may remember. A counter kept in a
    /// file does not: it starts over when the file is lost. `salon-host` draws
    /// a random number at each start. Not 0, which means "no run yet".
    pub epoch: Epoch,
    pub tv_key: TvKey,
    pub timing: Timing,
    pub limits: Limits,
    /// Devices remembered from a previous run ([`Host::known`]): they come
    /// back as themselves.
    pub known: Vec<Known>,
}

/// A device's identity, to keep across a restart of the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub device: DeviceId,
    pub token: Token,
    pub kind: Kind,
    pub name: Option<Name>,
}

/// What happened to a device, for the game.
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent<Up> {
    /// A device the room did not know. Send it what it should show.
    Arrived {
        device: DeviceId,
        kind: Kind,
        name: Option<Name>,
    },
    /// A device that had left is here again. Salon already resends its
    /// latest state and its pending events; a game may refresh them.
    Back { device: DeviceId },
    /// The device is gone, for now. It keeps its identity and may come back.
    Left { device: DeviceId, why: Why },
    /// An event, once and in order. `phase` is the phase the device had seen
    /// when it sent it (see [`Host::set_phase`]): the game decides whether it
    /// is stale. `at` is when it was sent, on the host's clock, as the device
    /// estimates it, bounded to the last round trip so nobody can backdate.
    Event {
        device: DeviceId,
        event: Up,
        phase: u32,
        at: Ms,
    },
    /// The device's latest state. Always newer than the one before.
    State { device: DeviceId, state: Up },
}

/// What the shell must do, or what the game must know.
#[derive(Clone, Debug, PartialEq)]
pub enum Output<Up> {
    Transmit {
        link: LinkId,
        bytes: Vec<u8>,
    },
    /// Close this transport; salon is done with it.
    Close {
        link: LinkId,
    },
    Device(DeviceEvent<Up>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// No such device (never was, or forgotten).
    Unknown,
    /// A gamepad or the keyboard: nothing to send to.
    Local,
    /// Encoded bigger than [`Limits::max_down`].
    TooBig,
    /// [`Limits::max_pending`] events, or 1 MiB of them, already wait for this
    /// device.
    Full,
}

/// A device, as the game sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub kind: Kind,
    pub name: Option<Name>,
    pub presence: Presence,
    /// Smoothed round trip, in milliseconds, once measured.
    pub rtt: Option<u64>,
    pub last_heard: Ms,
    /// When the host last knew the device still holds its latest state (an
    /// input held down, say). A game drops a held input that is too old.
    pub held_at: Option<Ms>,
    /// Messages from this device that made no sense, ignored.
    pub garbage: u32,
}

/// What happened to a local device's cable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plug {
    In,
    Out,
}

struct Device {
    kind: Kind,
    token: Option<Token>,
    name: Option<Name>,
    link: Option<LinkId>,
    presence: Presence,
    last_heard: Ms,
    peer: Peer,
    window: Window,
    up: InEvents,
    down: OutEvents,
    out_state: OutState,
    in_state: InState,
    events: Bucket,
    held_at: Option<Ms>,
    garbage: u32,
    /// The game has heard of it in this run. Devices remembered from a
    /// previous run are not: their first connection is an arrival.
    announced: bool,
    /// Which client holds the numbering (see [`Resume`]).
    session: u32,
    /// Where the numbers of events stood when it began.
    session_began: u32,
    /// Past sessions, with the number the events stood at when each ended.
    past: VecDeque<(u32, u32)>,
}

impl Device {
    fn new(now: Ms, kind: Kind, token: Option<Token>, name: Option<Name>, limits: &Limits) -> Self {
        Device {
            kind,
            token,
            name,
            link: None,
            presence: Presence::Absent,
            last_heard: now,
            peer: Peer::new(),
            window: Window::new(),
            up: InEvents::new(0),
            down: OutEvents::new(0),
            out_state: OutState::new(),
            in_state: InState::new(),
            events: Bucket::new(now, limits.events_per_s, limits.events_burst),
            held_at: None,
            garbage: 0,
            announced: true,
            session: 0,
            session_began: 0,
            past: VecDeque::new(),
        }
    }

    /// How long a fragment waits for news on a link of this `order`.
    fn wait(&self, order: Order, timing: &Timing) -> u64 {
        order.wait(&self.peer, &self.window, timing)
    }

    /// Data bytes the window lets out now.
    fn room(&self, now: Ms, wait: u64, packet: usize) -> usize {
        let flying = self
            .down
            .flying(now, wait)
            .saturating_add(self.out_state.flying(now, wait));
        self.window.room(flying, packet)
    }

    /// Another client took over: what it sends is not the rest of a message
    /// the last one left half sent.
    fn new_session(&mut self) {
        let next = self.up.next();
        if next != self.session_began {
            if self.past.len() >= PAST_SESSIONS {
                self.past.pop_front();
            }
            self.past.push_back((self.session, next));
        }
        self.session = self.session.wrapping_add(1);
        self.session_began = next;
        self.up.abandon();
        self.in_state.abandon();
    }

    /// Below this number, the events of `session` arrived; 0 if unknown.
    fn seen(&self, session: u32) -> u32 {
        if session == self.session {
            return self.up.next();
        }
        self.past
            .iter()
            .find(|(past, _)| *past == session)
            .map_or(0, |(_, end)| *end)
    }

    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            kind: self.kind,
            name: self.name.clone(),
            presence: self.presence,
            rtt: self.peer.rtt(),
            last_heard: self.last_heard,
            held_at: self.held_at,
            garbage: self.garbage,
        }
    }
}

struct Link {
    caps: Caps,
    device: Option<DeviceId>,
    opened: Ms,
    /// Rank among the links, by opening: two can open at the same `Ms`.
    rank: u64,
    last_sent: Option<Ms>,
    packets: Bucket,
    garbage: u32,
    controls: VecDeque<Control>,
    /// Close once `controls` are sent and the grace is over.
    closing: bool,
    close_at: Option<Ms>,
    ack_due: bool,
}

/// The room's side of salon. See the crate documentation for how it works.
pub struct Host<Up, Down> {
    config: HostConfig,
    now: Ms,
    devices: BTreeMap<DeviceId, Device>,
    tokens: BTreeMap<Token, DeviceId>,
    links: BTreeMap<LinkId, Link>,
    next_rank: u64,
    next_device: u32,
    phase: u32,
    open: bool,
    happened: VecDeque<DeviceEvent<Up>>,
    outbox: VecDeque<Output<Up>>,
    _down: PhantomData<fn(Down)>,
}

impl<Up: DeserializeOwned, Down: Serialize> Host<Up, Down> {
    pub fn new(now: Ms, mut config: HostConfig) -> Self {
        config.limits = config.limits.bounded();
        config.timing = config.timing.bounded();
        let mut devices = BTreeMap::new();
        let mut tokens = BTreeMap::new();
        let mut next_device = 1;
        for known in &config.known {
            if !known.kind.is_remote() || tokens.contains_key(&known.token) {
                continue;
            }
            let mut device = Device::new(
                now,
                known.kind,
                Some(known.token),
                known.name.clone(),
                &config.limits,
            );
            device.announced = false;
            devices.insert(known.device, device);
            tokens.insert(known.token, known.device);
            // Identities only grow, even across restarts.
            next_device = next_device.max(known.device.0.saturating_add(1));
        }
        Host {
            config,
            now,
            devices,
            tokens,
            links: BTreeMap::new(),
            next_rank: 0,
            next_device,
            phase: 0,
            open: true,
            happened: VecDeque::new(),
            outbox: VecDeque::new(),
            _down: PhantomData,
        }
    }

    fn advance(&mut self, now: Ms) -> Ms {
        // Time never goes back (str0m does the same).
        self.now = self.now.max(now);
        self.now
    }

    /// A transport opened. Nothing is sent on it until it says who it is.
    pub fn opened(&mut self, now: Ms, link: LinkId, caps: Caps) {
        let now = self.advance(now);
        let limits = self.config.limits;
        let rank = self.next_rank;
        self.next_rank = rank.saturating_add(1);
        self.links.entry(link).or_insert(Link {
            caps,
            device: None,
            opened: now,
            rank,
            last_sent: None,
            packets: Bucket::new(now, limits.packets_per_s, limits.packets_burst),
            garbage: 0,
            controls: VecDeque::new(),
            closing: false,
            close_at: None,
            ack_due: false,
        });
    }

    /// The transport closed (or failed). Its device, if it was the current
    /// one, has left.
    pub fn closed(&mut self, now: Ms, link: LinkId) {
        self.advance(now);
        if let Some(gone) = self.links.remove(&link) {
            self.detach(gone.device, link, Why::Closed);
        }
    }

    /// Bytes from a link. Anything that makes no sense is ignored.
    pub fn received(&mut self, now: Ms, link: LinkId, bytes: &[u8]) {
        let now = self.advance(now);
        let Some(current) = self.links.get_mut(&link) else {
            return;
        };
        // Told no, or replaced: a late copy of a hello must not bring it back
        // (socket.io 46213a6).
        if current.closing {
            return;
        }
        // A bigger packet closes its link, and only that one.
        if bytes.len() > MAX_PACKET {
            self.drop_link(link, Why::Closed);
            return;
        }
        if !current.packets.allow(now) {
            return;
        }
        let Some(packet) = wire::decode(bytes) else {
            if current.device.is_none() && wire::another_protocol(bytes) {
                self.refuse(link, Refusal::Version);
                return;
            }
            current.garbage = current.garbage.saturating_add(1);
            if current.device.is_none() && current.garbage >= GARBAGE_BEFORE_HELLO {
                self.drop_link(link, Why::Closed);
            } else if let Some(device) = current.device.and_then(|id| self.devices.get_mut(&id)) {
                device.garbage = device.garbage.saturating_add(1);
            }
            return;
        };
        match packet.control.clone() {
            Some(Control::Hello(hello)) => self.hello(now, link, &hello),
            Some(Control::Bye) => {
                // What it carries was sent before it.
                self.data(now, link, packet);
                self.drop_link(link, Why::Quit);
                return;
            }
            _ => {}
        }
        self.data(now, link, packet);
    }

    fn data(&mut self, now: Ms, link: LinkId, packet: Packet) {
        let Some(current) = self.links.get_mut(&link) else {
            return;
        };
        let Some(id) = current.device else {
            return;
        };
        if packet.epoch != self.config.epoch.0 {
            // Numbered for another run of the host: meaningless now.
            return;
        }
        let Some(device) = self.devices.get_mut(&id) else {
            return;
        };
        let max_up = self.config.limits.max_up;
        device.last_heard = now;
        device.presence = Presence::Present;
        device.peer.on_packet(now, packet.sent, packet.echo);
        if let Some(ack) = packet.ack {
            let heard = packet.echo.map(|echo| Ms(echo.sent));
            let order = Order::of(current.caps);
            let round = device.window.round(&device.peer, &self.config.timing);
            let learned = device.down.ack(&ack, heard, order);
            let learned = learned.with(device.out_state.ack(&ack, heard, order));
            device.window.heard(learned, now, round);
        }
        let had_parts = !packet.events.is_empty() || !packet.state.is_empty();
        let received = device
            .up
            .receive(packet.events, max_up, || device.events.allow(now));
        let (state, state_garbage) = device.in_state.receive(packet.state, max_up);
        if let Some(held) = packet.holding
            && held > 0
            && held == device.in_state.applied()
        {
            device.held_at = Some(now);
        }
        device.garbage = device
            .garbage
            .saturating_add(received.garbage.saturating_add(state_garbage));
        let rtt = device.peer.rtt().unwrap_or(0);
        for arrived in received.events {
            match crate::codec::decode::<Up>(&arrived.data) {
                Ok(event) => {
                    let earliest = Ms(now.0.saturating_sub(rtt));
                    let at = arrived.at.map_or(now, |at| Ms(at).clamp(earliest, now));
                    self.happened.push_back(DeviceEvent::Event {
                        device: id,
                        event,
                        phase: arrived.phase,
                        at,
                    });
                }
                Err(_) => device.garbage = device.garbage.saturating_add(1),
            }
        }
        if let Some(data) = state {
            device.held_at = Some(now);
            match crate::codec::decode::<Up>(&data) {
                Ok(state) => self
                    .happened
                    .push_back(DeviceEvent::State { device: id, state }),
                Err(_) => device.garbage = device.garbage.saturating_add(1),
            }
        }
        if had_parts || received.progressed {
            current.ack_due = true;
        }
    }

    fn hello(&mut self, now: Ms, link: LinkId, hello: &Hello) {
        let Some(current) = self.links.get(&link) else {
            return;
        };
        let local = current.caps.local;
        if let Some(id) = current.device {
            // Already welcomed here: the welcome was lost, say it again.
            if self.devices.get(&id).and_then(|device| device.token) == Some(hello.token) {
                self.welcome(link, id, hello.resume);
            }
            return;
        }
        // Everything is checked before anything changes (Colyseus #962).
        if hello.protocol != PROTOCOL || hello.game != self.config.game {
            self.refuse(link, Refusal::Version);
            return;
        }
        let false_tv = hello.kind == Kind::Tv && (hello.key != Some(self.config.tv_key) || !local);
        if !hello.kind.is_remote() || false_tv {
            self.refuse(link, Refusal::NotAllowed);
            return;
        }
        let name = hello.name.as_deref().and_then(Name::new);
        if let Some(&id) = self.tokens.get(&hello.token) {
            self.back(now, link, id, hello, name);
            return;
        }
        if !self.open {
            self.refuse(link, Refusal::Closed);
            return;
        }
        let connected = self
            .devices
            .values()
            .filter(|device| device.link.is_some())
            .count();
        if connected >= self.config.limits.max_devices {
            self.refuse(link, Refusal::Full);
            return;
        }
        let id = DeviceId(self.next_device);
        self.next_device = self.next_device.saturating_add(1);
        let mut device = Device::new(
            now,
            hello.kind,
            Some(hello.token),
            name.clone(),
            &self.config.limits,
        );
        device.link = Some(link);
        device.presence = Presence::Present;
        self.devices.insert(id, device);
        self.tokens.insert(hello.token, id);
        if let Some(current) = self.links.get_mut(&link) {
            current.device = Some(id);
        }
        self.happened.push_back(DeviceEvent::Arrived {
            device: id,
            kind: hello.kind,
            name,
        });
        self.welcome(link, id, None);
    }

    /// A device the room knows shows its token on `link`.
    fn back(&mut self, now: Ms, link: LinkId, id: DeviceId, hello: &Hello, name: Option<Name>) {
        let Some(device) = self.devices.get(&id) else {
            return;
        };
        if device.kind != hello.kind && (device.kind == Kind::Tv || hello.kind == Kind::Tv) {
            self.refuse(link, Refusal::NotAllowed);
            return;
        }
        if let Some(old) = device.link
            && old != link
        {
            // The newest link wins, not the hello that came last. One from an
            // older link may be the device's own: a try it gave up on, or the
            // page before a reload, handed over late by TCP. Whatever still
            // listens there is the tab that lost.
            if self.rank(link) < self.rank(old) {
                self.replace(link);
                return;
            }
            self.replace(old);
        }
        let Some(device) = self.devices.get_mut(&id) else {
            return;
        };
        let back = device.presence == Presence::Absent;
        let first = !device.announced;
        device.announced = true;
        device.kind = hello.kind;
        device.link = Some(link);
        device.presence = Presence::Present;
        device.last_heard = now;
        device.peer.reset();
        device.window = Window::new();
        device.down.unsend();
        device.out_state.unsend();
        let session = Resume {
            epoch: self.config.epoch.0,
            device: id,
            session: device.session,
        };
        if hello.resume != Some(session) {
            device.new_session();
        }
        if name.is_some() {
            device.name = name;
        }
        if let Some(current) = self.links.get_mut(&link) {
            current.device = Some(id);
        }
        if first {
            let (kind, name) = (device.kind, device.name.clone());
            self.happened.push_back(DeviceEvent::Arrived {
                device: id,
                kind,
                name,
            });
        } else if back {
            self.happened.push_back(DeviceEvent::Back { device: id });
        }
        self.welcome(link, id, hello.resume);
    }

    fn welcome(&mut self, link: LinkId, id: DeviceId, resume: Option<Resume>) {
        let Some(device) = self.devices.get(&id) else {
            return;
        };
        let epoch = self.config.epoch.0;
        let welcome = Welcome {
            device: id,
            epoch,
            session: device.session,
            up_next: device.up.next(),
            up_seen: resume
                .filter(|resume| resume.epoch == epoch && resume.device == id)
                .map_or(0, |resume| device.seen(resume.session)),
            state: device.in_state.applied(),
            down_first: device.down.oldest(),
            max_up: u32::try_from(self.config.limits.max_up).unwrap_or(u32::MAX),
            max_down: u32::try_from(self.config.limits.max_down).unwrap_or(u32::MAX),
        };
        if let Some(current) = self.links.get_mut(&link) {
            current.controls.push_back(Control::Welcome(welcome));
        }
    }

    fn final_message(&mut self, link: LinkId, control: &Control) {
        if let Some(current) = self.links.get_mut(&link) {
            for _ in 0..FINAL_COPIES {
                current.controls.push_back(control.clone());
            }
            current.closing = true;
        }
    }

    fn refuse(&mut self, link: LinkId, refusal: Refusal) {
        self.final_message(link, &Control::Refused(refusal));
    }

    fn rank(&self, link: LinkId) -> Option<u64> {
        self.links.get(&link).map(|link| link.rank)
    }

    /// Another link took this device: the old one is told, then closed. The
    /// device never left, so the game hears nothing.
    fn replace(&mut self, old: LinkId) {
        if let Some(current) = self.links.get_mut(&old) {
            current.device = None;
        }
        self.final_message(old, &Control::Replaced);
    }

    /// Detaches `device` from `link` if it is still its current link.
    fn detach(&mut self, device: Option<DeviceId>, link: LinkId, why: Why) {
        let Some(id) = device else {
            return;
        };
        let Some(device) = self.devices.get_mut(&id) else {
            return;
        };
        // A late close of a replaced link must not touch the new one
        // (Colyseus #950).
        if device.link != Some(link) {
            return;
        }
        device.link = None;
        device.presence = Presence::Absent;
        self.happened
            .push_back(DeviceEvent::Left { device: id, why });
    }

    fn drop_link(&mut self, link: LinkId, why: Why) {
        if let Some(gone) = self.links.remove(&link) {
            self.outbox.push_back(Output::Close { link });
            self.detach(gone.device, link, why);
        }
    }

    /// Time passed: silent devices weaken, then leave; strangers that never
    /// said hello are dropped.
    pub fn tick(&mut self, now: Ms) {
        let now = self.advance(now);
        let timing = self.config.timing;
        let strangers: Vec<LinkId> = self
            .links
            .iter()
            .filter(|(_, link)| {
                link.device.is_none() && !link.closing && now.since(link.opened) >= timing.handshake
            })
            .map(|(id, _)| *id)
            .collect();
        for link in strangers {
            self.drop_link(link, Why::Closed);
        }
        let mut silent = Vec::new();
        for device in self.devices.values_mut() {
            let Some(link) = device.link else {
                continue;
            };
            let quiet = now.since(device.last_heard);
            let stuck = self.links.get(&link).is_some_and(|link| {
                let order = Order::of(link.caps);
                order.stuck(now, &mut device.down, &mut device.out_state, &timing)
            });
            if quiet >= timing.absent || stuck {
                silent.push(link);
            } else if quiet >= timing.silent {
                device.presence = Presence::Weak;
            }
        }
        for link in silent {
            self.drop_link(link, Why::Silent);
        }
    }

    /// The next thing to do or know, `None` when nothing is left for now.
    /// Call it until `None` after every other call, and at
    /// [`Host::next_deadline`]: it runs [`Host::tick`] itself.
    pub fn poll(&mut self, now: Ms) -> Option<Output<Up>> {
        let now = self.advance(now);
        if self.happened.is_empty() && self.outbox.is_empty() {
            self.tick(now);
        }
        if let Some(event) = self.happened.pop_front() {
            return Some(Output::Device(event));
        }
        if self.outbox.is_empty() {
            self.flush(now);
        }
        self.outbox.pop_front()
    }

    fn flush(&mut self, now: Ms) {
        let timing = self.config.timing;
        let epoch = self.config.epoch.0;
        let mut finished = Vec::new();
        for (&id, link) in &mut self.links {
            let mut device = link.device.and_then(|device| self.devices.get_mut(&device));
            let packet_size = link.caps.packet();
            let order = Order::of(link.caps);
            let wait = device
                .as_deref()
                .map_or(0, |device| device.wait(order, &timing));
            let mut room = device
                .as_deref()
                .map_or(0, |device| device.room(now, wait, packet_size));
            for _ in 0..BURST {
                let control = link.controls.pop_front();
                let mut packet = Packet {
                    epoch,
                    sent: now.0,
                    control,
                    ..Packet::default()
                };
                let mut has_data = false;
                if let Some(device) = device.as_deref_mut() {
                    let mut budget = Budget::for_packet(packet_size, room);
                    budget.bytes =
                        budget
                            .bytes
                            .saturating_sub(if packet.control.is_some() { 64 } else { 0 });
                    packet.echo = device.peer.echo(now);
                    packet.phase = Some(self.phase);
                    packet.ack = Some(Ack {
                        events: device.up.next(),
                        state: device.in_state.applied(),
                        ahead: device.up.ahead(),
                        event_parts: device.up.parts(),
                        state_parts: device.in_state.parts(),
                    });
                    packet.events = device.down.take(now, wait, packet_size, &mut budget);
                    packet.state = device.out_state.take(now, wait, packet_size, &mut budget);
                    has_data = !packet.events.is_empty() || !packet.state.is_empty();
                    room = budget.room;
                    if budget.expired {
                        device.window.expired(now, wait);
                    }
                } else if packet.control.is_none() {
                    break;
                }
                let heartbeat = device.is_some()
                    && link
                        .last_sent
                        .is_none_or(|at| now.since(at) >= timing.heartbeat);
                if packet.control.is_none() && !has_data && !link.ack_due && !heartbeat {
                    break;
                }
                if let Some(bytes) = wire::encode_within(packet, packet_size) {
                    self.outbox.push_back(Output::Transmit { link: id, bytes });
                }
                link.last_sent = Some(now);
                link.ack_due = false;
            }
            if link.closing && link.controls.is_empty() {
                let close_at = *link.close_at.get_or_insert(now.plus(FINAL_GRACE));
                if close_at <= now {
                    finished.push(id);
                }
            }
        }
        for link in finished {
            self.drop_link(link, Why::Closed);
        }
    }

    /// When to call [`Host::tick`] and [`Host::poll`] again at the latest.
    pub fn next_deadline(&self) -> Option<Ms> {
        let timing = self.config.timing;
        let mut next: Option<Ms> = None;
        let mut consider = |at: Ms| next = Some(next.map_or(at, |next| next.min(at)));
        if !self.happened.is_empty() || !self.outbox.is_empty() {
            consider(self.now);
        }
        for link in self.links.values() {
            if !link.controls.is_empty() || link.ack_due {
                consider(self.now);
            }
            if let Some(at) = link.close_at {
                consider(at);
                continue;
            }
            match link.device.and_then(|id| self.devices.get(&id)) {
                None => consider(link.opened.plus(timing.handshake)),
                Some(device) => {
                    consider(
                        link.last_sent
                            .map_or(self.now, |at| at.plus(timing.heartbeat)),
                    );
                    let quiet = if device.presence == Presence::Present {
                        timing.silent
                    } else {
                        timing.absent
                    };
                    consider(device.last_heard.plus(quiet));
                    let wait = device.wait(Order::of(link.caps), &timing);
                    let room = device.room(self.now, wait, link.caps.packet());
                    if let Some(at) = device.down.due_at(self.now, wait, room) {
                        consider(at.max(self.now));
                    }
                    if let Some(at) = device.out_state.due_at(self.now, wait, room) {
                        consider(at.max(self.now));
                    }
                }
            }
        }
        next
    }

    fn encode<T: Serialize>(&self, message: &T) -> Result<Vec<u8>, SendError> {
        let data = postcard::to_allocvec(message).map_err(|_| SendError::TooBig)?;
        if data.len() > self.config.limits.max_down {
            return Err(SendError::TooBig);
        }
        Ok(data)
    }

    fn remote(&mut self, device: DeviceId) -> Result<&mut Device, SendError> {
        let device = self.devices.get_mut(&device).ok_or(SendError::Unknown)?;
        if device.kind.is_remote() {
            Ok(device)
        } else {
            Err(SendError::Local)
        }
    }

    /// An event for `device`: delivered once, in order, even across its
    /// reconnections. Waits while the device is away.
    pub fn send(&mut self, device: DeviceId, event: &Down) -> Result<(), SendError> {
        let data = self.encode(event)?;
        let phase = self.phase;
        let max = self.config.limits.max_pending;
        self.remote(device)?
            .down
            .push(data, phase, None, max)
            .map(|_| ())
            .map_err(|_| SendError::Full)
    }

    /// What `device` should show now. Replaces the previous state, unless that
    /// one is on its way in several packets: it finishes, then the latest
    /// follows. Resent until the device has it, and again on each reconnection.
    pub fn show(&mut self, device: DeviceId, state: &Down) -> Result<(), SendError> {
        let data = self.encode(state)?;
        self.remote(device)?.out_state.set(data);
        Ok(())
    }

    /// The game's phase (a round, a question). Every event a device sends
    /// carries the phase it had seen, so the game can drop a late answer.
    pub fn set_phase(&mut self, phase: u32) {
        self.phase = phase;
    }

    pub fn phase(&self) -> u32 {
        self.phase
    }

    /// How many identities were given so far: when it moves, [`Host::known`]
    /// has something to save.
    pub fn identities(&self) -> u32 {
        self.next_device
    }

    /// Unknown devices may no longer join. Known ones always come back.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Unknown devices may join again, as at the start.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Removes a device for good. If connected, it is told
    /// ([`Refusal::Removed`]). Its token names nobody any more: the phone
    /// that shows it again is a stranger, and arrives as a new device, unless
    /// the room is closed ([`Host::close`]). Its number is not given again in
    /// this run; after a restart it can be, since [`Host::known`] no longer
    /// has it. What the phone had not seen acknowledged goes again under its
    /// new number, so an event can reach the game once under each.
    pub fn forget(&mut self, device: DeviceId) {
        let Some(gone) = self.devices.remove(&device) else {
            return;
        };
        if let Some(token) = gone.token {
            self.tokens.remove(&token);
        }
        if let Some(link) = gone.link {
            if let Some(current) = self.links.get_mut(&link) {
                current.device = None;
            }
            self.refuse(link, Refusal::Removed);
        }
    }

    /// Tells every device the host stops. Poll until `None`, then exit.
    pub fn shutdown(&mut self) {
        let links: Vec<LinkId> = self.links.keys().copied().collect();
        for link in links {
            if let Some(current) = self.links.get_mut(&link) {
                current.device = None;
            }
            self.final_message(link, &Control::Closing);
        }
    }

    pub fn device(&self, device: DeviceId) -> Option<DeviceInfo> {
        self.devices.get(&device).map(Device::info)
    }

    pub fn devices(&self) -> impl Iterator<Item = (DeviceId, DeviceInfo)> + '_ {
        self.devices.iter().map(|(id, device)| (*id, device.info()))
    }

    /// Identities to keep, so devices come back as themselves after a
    /// restart ([`HostConfig::known`]).
    pub fn known(&self) -> Vec<Known> {
        self.devices
            .iter()
            .filter_map(|(id, device)| {
                Some(Known {
                    device: *id,
                    token: device.token?,
                    kind: device.kind,
                    name: device.name.clone(),
                })
            })
            .collect()
    }

    /// A gamepad or the keyboard, read by the host itself, joins as a device
    /// like the others.
    pub fn add_local(&mut self, now: Ms, kind: Kind, name: Option<Name>) -> DeviceId {
        let now = self.advance(now);
        let id = DeviceId(self.next_device);
        self.next_device = self.next_device.saturating_add(1);
        let mut device = Device::new(now, kind, None, name.clone(), &self.config.limits);
        device.presence = Presence::Present;
        self.devices.insert(id, device);
        self.happened.push_back(DeviceEvent::Arrived {
            device: id,
            kind,
            name,
        });
        id
    }

    /// A local device was unplugged.
    pub fn local_unplugged(&mut self, now: Ms, device: DeviceId) {
        self.local_plug(now, device, Plug::Out);
    }

    /// A local device was plugged back.
    pub fn local_plugged(&mut self, now: Ms, device: DeviceId) {
        self.local_plug(now, device, Plug::In);
    }

    fn local_plug(&mut self, now: Ms, device: DeviceId, plug: Plug) {
        let now = self.advance(now);
        let Some(local) = self.devices.get_mut(&device) else {
            return;
        };
        let plugged = local.presence != Presence::Absent;
        if local.kind.is_remote() || plugged == (plug == Plug::In) {
            return;
        }
        let (presence, event) = match plug {
            Plug::In => (Presence::Present, DeviceEvent::Back { device }),
            Plug::Out => (
                Presence::Absent,
                DeviceEvent::Left {
                    device,
                    why: Why::Closed,
                },
            ),
        };
        local.presence = presence;
        local.last_heard = now;
        self.happened.push_back(event);
    }

    /// An event from a local device.
    pub fn local_event(&mut self, now: Ms, device: DeviceId, event: Up) {
        let now = self.advance(now);
        if self
            .devices
            .get(&device)
            .is_some_and(|local| !local.kind.is_remote())
        {
            let phase = self.phase;
            self.happened.push_back(DeviceEvent::Event {
                device,
                event,
                phase,
                at: now,
            });
        }
    }

    /// The latest state of a local device.
    pub fn local_state(&mut self, now: Ms, device: DeviceId, state: Up) {
        let now = self.advance(now);
        if let Some(local) = self.devices.get_mut(&device)
            && !local.kind.is_remote()
        {
            local.held_at = Some(now);
            local.last_heard = now;
            self.happened
                .push_back(DeviceEvent::State { device, state });
        }
    }
}
