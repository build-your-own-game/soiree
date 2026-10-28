//! The device side: a phone, the app or the TV page runs one `Client`.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::stream::{Budget, InEvents, InState, OutEvents, OutState, Peer};
use crate::types::{Caps, DeviceId, Kind, Limits, Ms, Refusal, Timing, Token, TvKey};
use crate::wire::{self, Ack, Control, Hello, PROTOCOL, Packet, Welcome};

/// Packets one flush may send, to drain a large message.
const BURST: usize = 32;
/// Copies of the goodbye: one lost packet must not leave a ghost.
const BYE_COPIES: usize = 3;

pub struct ClientConfig {
    /// The game's version, checked by the host.
    pub game: u32,
    pub kind: Kind,
    /// Drawn once from the OS's secure random generator and kept (the shell
    /// stores it): it is this device's identity.
    pub token: Token,
    pub name: Option<String>,
    /// Only for the TV, from the page the host served.
    pub tv_key: Option<TvKey>,
    /// Any number that differs between devices: it spreads reconnections.
    pub seed: u64,
    pub timing: Timing,
    pub limits: Limits,
}

/// What the player should be told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// First connection, not welcomed yet.
    Connecting,
    Connected,
    /// The link died; salon is reconnecting on its own, without end.
    Reconnecting,
    /// The host said it stops; salon keeps trying to come back.
    HostGone,
    /// Another tab or phone took this device over. Salon does not reconnect
    /// on its own: offer "play here" ([`Client::resume`]).
    Replaced,
    /// The host turned this device away. Show why; [`Client::resume`] tries
    /// again.
    Refused(Refusal),
    /// The player left ([`Client::quit`]).
    Quit,
}

/// What the shell must do, or what the page must know.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientOutput<Down> {
    /// Open a transport to the host, then call [`Client::opened`] (or
    /// [`Client::closed`] if it fails).
    Connect,
    Transmit(Vec<u8>),
    /// Close the current transport; salon is done with it.
    Disconnect,
    Status(Status),
    /// The host restarted or forgot this device: whatever it said before is
    /// void. Wait for its new state.
    Reset,
    /// An event from the host, once and in order.
    Event(Down),
    /// What to show now. Always newer than the one before.
    State(Down),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// Encoded bigger than [`Limits::max_up`].
    TooBig,
    /// [`Limits::max_pending`] events already wait: the host is unreachable
    /// or flooded. Nothing was dropped; tell the player.
    Full,
}

enum Link {
    /// No transport. `None`: stopped until [`Client::resume`].
    Idle {
        retry_at: Option<Ms>,
    },
    Connecting {
        since: Ms,
    },
    Open(Open),
}

/// When [`Client::retry`] may try again.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Retry {
    /// At once, unless an attempt already failed: then after the backoff.
    Now,
    /// After the backoff.
    Later,
}

struct Open {
    caps: Caps,
    opened: Ms,
    last_heard: Ms,
    last_sent: Option<Ms>,
    welcomed: bool,
    ack_due: bool,
    /// Goodbyes still to send, then the link closes.
    bye: usize,
}

/// A device's side of salon. See the crate documentation for how it works.
pub struct Client<Up, Down> {
    config: ClientConfig,
    now: Ms,
    link: Link,
    status: Status,
    /// The host's epoch and this device's identity, once welcomed.
    known: Option<(u32, DeviceId)>,
    phase: u32,
    up: OutEvents,
    down: InEvents,
    out_state: OutState,
    in_state: InState,
    peer: Peer,
    attempts: u32,
    rng: u64,
    outbox: VecDeque<ClientOutput<Down>>,
    _up: PhantomData<fn(Up)>,
}

impl<Up: Serialize, Down: DeserializeOwned> Client<Up, Down> {
    pub fn new(now: Ms, config: ClientConfig) -> Self {
        let rng = config.seed ^ 0x9e37_79b9_7f4a_7c15;
        Client {
            config,
            now,
            link: Link::Idle {
                retry_at: Some(now),
            },
            status: Status::Connecting,
            known: None,
            phase: 0,
            up: OutEvents::new(0),
            down: InEvents::new(0),
            out_state: OutState::new(),
            in_state: InState::new(),
            peer: Peer::new(),
            attempts: 0,
            rng,
            outbox: VecDeque::new(),
            _up: PhantomData,
        }
    }

    fn advance(&mut self, now: Ms) -> Ms {
        self.now = self.now.max(now);
        self.now
    }

    fn set_status(&mut self, status: Status) {
        if self.status != status {
            self.status = status;
            self.outbox.push_back(ClientOutput::Status(status));
        }
    }

    /// The wait before the next attempt: doubling, capped, ±25 % of chance.
    fn backoff(&mut self) -> u64 {
        let timing = self.config.timing;
        let base = timing
            .retry_first
            .saturating_mul(1 << self.attempts.min(16))
            .min(timing.retry_max);
        self.attempts = self.attempts.saturating_add(1);
        // SplitMix64: enough to spread phones apart.
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        let spread = base / 2;
        let jitter = z.checked_rem(spread).unwrap_or(0);
        base.saturating_sub(base / 4).saturating_add(jitter)
    }

    /// Drops the current transport and plans the next attempt.
    fn retry(&mut self, now: Ms, when: Retry) {
        if let Link::Open(open) = &self.link {
            if open.welcomed && now.since(open.opened) >= self.config.timing.healthy {
                self.attempts = 0;
            }
            self.outbox.push_back(ClientOutput::Disconnect);
        } else if matches!(self.link, Link::Connecting { .. }) {
            self.outbox.push_back(ClientOutput::Disconnect);
        }
        let wait = if when == Retry::Now && self.attempts == 0 {
            self.attempts = 1;
            0
        } else {
            self.backoff()
        };
        self.link = Link::Idle {
            retry_at: Some(now.plus(wait)),
        };
        if !matches!(self.status, Status::Connecting | Status::HostGone) {
            self.set_status(Status::Reconnecting);
        }
    }

    /// Stops until [`Client::resume`].
    fn stop(&mut self, status: Status) {
        if !matches!(self.link, Link::Idle { .. }) {
            self.outbox.push_back(ClientOutput::Disconnect);
        }
        self.link = Link::Idle { retry_at: None };
        self.set_status(status);
    }

    /// The transport asked for by [`ClientOutput::Connect`] is open.
    pub fn opened(&mut self, now: Ms, caps: Caps) {
        let now = self.advance(now);
        if matches!(self.link, Link::Connecting { .. }) {
            self.link = Link::Open(Open {
                caps,
                opened: now,
                last_heard: now,
                last_sent: None,
                welcomed: false,
                ack_due: false,
                bye: 0,
            });
        }
    }

    /// The transport closed or failed to open.
    pub fn closed(&mut self, now: Ms) {
        let now = self.advance(now);
        if let Link::Open(open) = &self.link
            && open.bye > 0
        {
            self.stop(Status::Quit);
            return;
        }
        if !matches!(self.link, Link::Idle { .. }) {
            // The shell already knows: no Disconnect for it.
            if let Link::Open(open) = &self.link
                && open.welcomed
                && now.since(open.opened) >= self.config.timing.healthy
            {
                self.attempts = 0;
            }
            let wait = self.backoff();
            self.link = Link::Idle {
                retry_at: Some(now.plus(wait)),
            };
            if !matches!(self.status, Status::Connecting | Status::HostGone) {
                self.set_status(Status::Reconnecting);
            }
        }
    }

    /// The page became visible again, or the network came back
    /// (`visibilitychange`, `pageshow`, `online`): check now instead of
    /// waiting for the watchdog. iOS freezes pages in the background.
    pub fn wake(&mut self, now: Ms) {
        let now = self.advance(now);
        let silent = self.config.timing.silent;
        match &self.link {
            Link::Open(open) if now.since(open.last_heard) >= silent => {
                self.attempts = 0;
                self.retry(now, Retry::Now);
            }
            Link::Idle { retry_at: Some(_) } => {
                self.attempts = 0;
                self.link = Link::Idle {
                    retry_at: Some(now),
                };
            }
            Link::Open(_) | Link::Idle { retry_at: None } | Link::Connecting { .. } => {}
        }
    }

    /// Bytes from the host.
    pub fn received(&mut self, now: Ms, bytes: &[u8]) {
        let now = self.advance(now);
        if !matches!(self.link, Link::Open(_)) {
            return;
        }
        let Some(packet) = wire::decode(bytes) else {
            return;
        };
        match packet.control {
            Some(Control::Welcome(welcome)) => self.welcome(now, welcome),
            Some(Control::Refused(refusal)) => {
                self.stop(Status::Refused(refusal));
                return;
            }
            Some(Control::Replaced) => {
                self.stop(Status::Replaced);
                return;
            }
            Some(Control::Closing) => {
                self.set_status(Status::HostGone);
                self.retry(now, Retry::Later);
                return;
            }
            _ => {}
        }
        let Link::Open(open) = &mut self.link else {
            return;
        };
        let Some((epoch, _)) = self.known else {
            return;
        };
        if !open.welcomed || packet.epoch != epoch {
            return;
        }
        open.last_heard = now;
        self.peer.on_packet(now, packet.sent, packet.echo);
        if let Some(ack) = packet.ack {
            self.up.ack(ack.events);
            self.out_state.ack(ack.state);
        }
        if let Some(phase) = packet.phase {
            self.phase = phase;
        }
        let max = self.config.limits.max_down;
        let had_parts = !packet.events.is_empty() || !packet.state.is_empty();
        let received = self.down.receive(packet.events, max, || true);
        for arrived in received.events {
            if let Ok(event) = crate::codec::decode::<Down>(&arrived.data) {
                self.outbox.push_back(ClientOutput::Event(event));
            }
        }
        let (state, _) = self.in_state.receive(packet.state, max);
        if let Some(data) = state
            && let Ok(state) = crate::codec::decode::<Down>(&data)
        {
            self.outbox.push_back(ClientOutput::State(state));
        }
        if had_parts && let Link::Open(open) = &mut self.link {
            open.ack_due = true;
        }
    }

    fn welcome(&mut self, now: Ms, welcome: Welcome) {
        let Link::Open(open) = &mut self.link else {
            return;
        };
        if open.welcomed {
            return;
        }
        open.welcomed = true;
        open.last_heard = now;
        let identity = (welcome.epoch, welcome.device);
        match self.known {
            Some(known) if known == identity => {
                // Same host run, same device: keep the numbering; what the
                // host already has is acknowledged, the rest goes again.
                self.up.ack(welcome.up_next);
                self.down.skip_to(welcome.down_first);
            }
            Some(_) => {
                // The host restarted or forgot us: what it said is void, and
                // what we had for it is meaningless now.
                self.outbox.push_back(ClientOutput::Reset);
                self.up.restart(welcome.up_next);
                self.down.restart(welcome.down_first);
                self.in_state.restart();
            }
            None => {
                // First welcome: what the player did while connecting counts.
                self.up.renumber(welcome.up_next);
                self.down.restart(welcome.down_first);
            }
        }
        self.known = Some(identity);
        self.up.unsend();
        self.out_state.unsend();
        self.peer.reset();
        self.set_status(Status::Connected);
    }

    /// Time passed: open, abandon or watch the transport.
    pub fn tick(&mut self, now: Ms) {
        let now = self.advance(now);
        let timing = self.config.timing;
        match &self.link {
            Link::Idle { retry_at: Some(at) } if *at <= now => {
                self.outbox.push_back(ClientOutput::Connect);
                self.link = Link::Connecting { since: now };
            }
            Link::Connecting { since } if now.since(*since) >= timing.connect => {
                self.retry(now, Retry::Later);
            }
            Link::Open(open) if !open.welcomed && now.since(open.opened) >= timing.handshake => {
                self.retry(now, Retry::Later);
            }
            // The page's own watchdog: the browser may never report a dead
            // socket (WebKit 247943), so silence is the only sure sign.
            Link::Open(open) if open.welcomed && now.since(open.last_heard) >= timing.absent => {
                self.retry(now, Retry::Now);
            }
            // Every state is named, so that a new one without its own
            // timeout breaks the build here instead of hanging forever.
            Link::Idle { .. } | Link::Connecting { .. } | Link::Open(_) => {}
        }
    }

    /// The next thing to do or know, `None` when nothing is left for now.
    pub fn poll(&mut self, now: Ms) -> Option<ClientOutput<Down>> {
        let now = self.advance(now);
        if self.outbox.is_empty() {
            self.tick(now);
            self.flush(now);
        }
        self.outbox.pop_front()
    }

    fn flush(&mut self, now: Ms) {
        let timing = self.config.timing;
        let Link::Open(open) = &mut self.link else {
            return;
        };
        let packet_size = open.caps.packet();
        let epoch = self.known.map_or(0, |(epoch, _)| epoch);
        let resend = self.peer.resend(timing.resend_min, timing.resend_max);
        let mut closing = false;
        for _ in 0..BURST {
            let mut packet = Packet {
                epoch,
                sent: now.0,
                ..Packet::default()
            };
            if let Some(left) = open.bye.checked_sub(1) {
                packet.control = Some(Control::Bye);
                open.bye = left;
                closing = left == 0;
            } else if !open.welcomed {
                if open.last_sent.is_some_and(|at| now.since(at) < resend) {
                    return;
                }
                packet.control = Some(Control::Hello(Hello {
                    protocol: PROTOCOL,
                    game: self.config.game,
                    kind: self.config.kind,
                    token: self.config.token,
                    name: self.config.name.clone(),
                    key: self.config.tv_key,
                    resume: self.known,
                }));
            }
            let mut has_data = false;
            if open.welcomed && !closing {
                let mut budget = Budget::for_packet(packet_size);
                packet.echo = self.peer.echo(now);
                packet.ack = Some(Ack {
                    events: self.down.next(),
                    state: self.in_state.applied(),
                });
                let held = self.out_state.seq();
                packet.holding = (held > 0).then_some(held);
                packet.events = self.up.take(now, resend, packet_size, &mut budget);
                packet.state = self.out_state.take(now, resend, packet_size, &mut budget);
                has_data = !packet.events.is_empty() || !packet.state.is_empty();
            }
            let heartbeat = open
                .last_sent
                .is_none_or(|at| now.since(at) >= timing.heartbeat);
            if packet.control.is_none() && !has_data && !open.ack_due && !heartbeat {
                break;
            }
            if let Some(bytes) = wire::encode_within(packet, packet_size) {
                self.outbox.push_back(ClientOutput::Transmit(bytes));
            }
            open.last_sent = Some(now);
            open.ack_due = false;
            if closing || (!has_data && open.bye == 0) {
                break;
            }
        }
        if closing {
            self.stop(Status::Quit);
        }
    }

    /// When to call [`Client::poll`] again at the latest.
    pub fn next_deadline(&self) -> Option<Ms> {
        if !self.outbox.is_empty() {
            return Some(self.now);
        }
        let timing = self.config.timing;
        match &self.link {
            Link::Idle { retry_at } => *retry_at,
            Link::Connecting { since } => Some(since.plus(timing.connect)),
            Link::Open(open) => {
                if open.ack_due || open.bye > 0 {
                    return Some(self.now);
                }
                let resend = self.peer.resend(timing.resend_min, timing.resend_max);
                let mut next = open
                    .last_sent
                    .map_or(self.now, |at| at.plus(timing.heartbeat));
                if open.welcomed {
                    next = next.min(open.last_heard.plus(timing.absent));
                    for due in [self.up.due_at(resend), self.out_state.due_at(resend)]
                        .into_iter()
                        .flatten()
                    {
                        next = next.min(due.max(self.now));
                    }
                } else {
                    next = next.min(open.last_sent.map_or(self.now, |at| at.plus(resend)));
                    next = next.min(open.opened.plus(timing.handshake));
                }
                Some(next)
            }
        }
    }

    fn encode(&self, message: &Up) -> Result<Vec<u8>, SendError> {
        let data = postcard::to_allocvec(message).map_err(|_| SendError::TooBig)?;
        if data.len() > self.config.limits.max_up {
            return Err(SendError::TooBig);
        }
        Ok(data)
    }

    /// An event for the host: delivered once, in order, even across
    /// reconnections. Kept while disconnected, up to [`Limits::max_pending`].
    pub fn send(&mut self, event: &Up) -> Result<(), SendError> {
        let data = self.encode(event)?;
        let at = self.peer.their_time(self.now);
        let max = self.config.limits.max_pending;
        self.up
            .push(data, self.phase, at, max)
            .map(|_| ())
            .map_err(|_| SendError::Full)
    }

    /// This device's latest state (its controls, say). Replaces the previous
    /// one; resent until the host has it.
    pub fn set_state(&mut self, state: &Up) -> Result<(), SendError> {
        let data = self.encode(state)?;
        self.out_state.set(data);
        Ok(())
    }

    /// The player leaves: the host is told, the transport closes.
    pub fn quit(&mut self) {
        match &mut self.link {
            Link::Open(open) if open.welcomed => {
                open.bye = BYE_COPIES;
                self.set_status(Status::Quit);
            }
            Link::Open(_) | Link::Idle { .. } | Link::Connecting { .. } => self.stop(Status::Quit),
        }
    }

    /// Connects again after [`Status::Replaced`], [`Status::Refused`] or
    /// [`Status::Quit`] ("play here").
    pub fn resume(&mut self) {
        if matches!(self.link, Link::Idle { retry_at: None }) {
            self.attempts = 0;
            self.link = Link::Idle {
                retry_at: Some(self.now),
            };
            self.set_status(if self.known.is_some() {
                Status::Reconnecting
            } else {
                Status::Connecting
            });
        }
    }

    /// The TV's key changed (the host restarted without keeping it): the
    /// next connection uses this one. Call [`Client::resume`] after it.
    pub fn set_tv_key(&mut self, key: TvKey) {
        self.config.tv_key = Some(key);
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// This device's identity in the room, once welcomed.
    pub fn device(&self) -> Option<DeviceId> {
        self.known.map(|(_, device)| device)
    }

    /// Smoothed round trip to the host, in milliseconds.
    pub fn rtt(&self) -> Option<u64> {
        self.peer.rtt()
    }

    /// The host's clock now, as measured (for countdowns in sync with the TV).
    pub fn host_time(&self, now: Ms) -> Option<Ms> {
        self.peer.their_time(now).map(Ms)
    }
}
