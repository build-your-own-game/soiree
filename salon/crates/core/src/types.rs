//! The small types every part of salon shares: time, identities, names,
//! timings and limits. Each is validated where it is built, so the rest of
//! the code never meets an impossible value.

use alloc::string::String;
use core::fmt;

use serde::{Deserialize, Serialize};

/// Milliseconds on a monotonic clock whose origin the shell chooses (the
/// start of the process, say). Time only enters salon through the calls, as
/// a `Ms`: never `Instant`, which panics in WASM and has no zero.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Ms(pub u64);

impl Ms {
    /// Milliseconds from `earlier` to `self`, zero if `earlier` is later.
    pub fn since(self, earlier: Ms) -> u64 {
        self.0.saturating_sub(earlier.0)
    }

    #[must_use]
    pub fn plus(self, ms: u64) -> Ms {
        Ms(self.0.saturating_add(ms))
    }
}

/// A device's public name, chosen by the host and never given to another
/// device in the same run (AirConsole's `device_id`). Games key everything
/// on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub u32);

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A device's secret: 128 random bits the phone draws once (from
/// `crypto.getRandomValues`, or the app's `SecRandom`), keeps, and sends on
/// every connection. Whoever holds it is that device; it is never sent to
/// another device.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Token(pub [u8; 16]);

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A secret stays out of logs.
        f.write_str("Token(…)")
    }
}

/// The secret that makes a connection the TV. The host draws it at start and
/// puts it only in the TV page it serves to its own computer.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TvKey(pub [u8; 16]);

impl fmt::Debug for TvKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TvKey(…)")
    }
}

/// One run of the host. It changes at each start, so a device that comes
/// back knows whether the host still remembers what it said.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(pub u32);

/// A transport connection, named by the shell. Disposable: a device keeps its
/// identity across many links.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub u64);

/// What a device is. The game decides who plays what; this only says what
/// the device can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// The shared screen, on the host's computer.
    Tv,
    /// A phone in a browser: touch, text, no motion sensors on `http://`.
    Phone,
    /// A phone in the remote app: sensors, haptics, UDP.
    App,
    /// A gamepad plugged into the host, read by the host itself.
    Gamepad,
    /// The host's keyboard.
    Keyboard,
}

impl Kind {
    /// Kinds that connect over the network (the others are the host's own).
    pub fn is_remote(self) -> bool {
        matches!(self, Kind::Tv | Kind::Phone | Kind::App)
    }
}

/// A device's display name: control characters removed, spaces trimmed,
/// at most [`Name::MAX_CHARS`] characters, never empty. Games apply their own
/// stricter rules on top (length on screen, uniqueness).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Name(String);

impl Name {
    pub const MAX_CHARS: usize = 32;

    pub fn new(raw: &str) -> Option<Name> {
        let cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control())
            .take(Name::MAX_CHARS * 4)
            .collect();
        let trimmed: String = cleaned.trim().chars().take(Name::MAX_CHARS).collect();
        let trimmed = trimmed.trim_end();
        (!trimmed.is_empty()).then(|| Name(String::from(trimmed)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a transport can carry in one piece. Salon cuts messages to fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    /// Largest packet the transport takes whole, in bytes.
    pub max_packet: usize,
    /// The link comes from the host's own computer (loopback). Only such a
    /// link can be the TV, key or not: a key replayed from the network is
    /// still refused.
    pub local: bool,
}

impl Caps {
    /// A WebRTC DataChannel without retransmission, or UDP: stay under one
    /// network frame, since losing one fragment loses the whole message
    /// (Quake 3 `FRAGMENT_SIZE`, str0m MTU 1150).
    pub const DATAGRAM: Caps = Caps {
        max_packet: 1100,
        local: false,
    };
    /// A WebSocket: nothing is lost, packets can be larger.
    pub const STREAM: Caps = Caps {
        max_packet: 16 * 1024,
        local: false,
    };
    /// Smallest packet salon works with (room for headers and a fragment).
    pub const MIN_PACKET: usize = 256;

    /// The same, from the host's own computer.
    #[must_use]
    pub const fn local(self) -> Caps {
        Caps {
            local: true,
            ..self
        }
    }

    pub(crate) fn packet(self) -> usize {
        self.max_packet.clamp(Caps::MIN_PACKET, 64 * 1024)
    }
}

/// Every delay salon waits on, in milliseconds. The defaults come from the
/// references in `salon/LEARNINGS.md`; games rarely change them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// Each side sends at least one packet this often, so silence means a
    /// dead link (Fiedler: regular traffic is the keepalive).
    pub heartbeat: u64,
    /// Silence after which a device is shown as on a weak link.
    pub silent: u64,
    /// Silence after which a device has left, and its link is dropped. The
    /// phone gives up on the host after the same silence and reconnects.
    pub absent: u64,
    /// A connection that has not said who it is by then is dropped.
    pub handshake: u64,
    /// Shortest and longest wait before resending what was not acknowledged;
    /// in between, twice the measured round trip.
    pub resend_min: u64,
    pub resend_max: u64,
    /// A transport that does not open by then is abandoned and tried again.
    pub connect: u64,
    /// Waits between reconnection attempts: the first, doubled up to the
    /// last, with ±25 % of chance so phones do not all retry together.
    pub retry_first: u64,
    pub retry_max: u64,
    /// A link that lived this long resets the waits (Colyseus `minUptime`).
    pub healthy: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            heartbeat: 250,
            silent: 1_500,
            absent: 6_000,
            handshake: 10_000,
            resend_min: 100,
            resend_max: 1_000,
            connect: 5_000,
            retry_first: 250,
            retry_max: 2_000,
            healthy: 5_000,
        }
    }
}

/// Every bound salon enforces. Nothing grows without one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Largest encoded message (event or state) a device may send.
    pub max_up: usize,
    /// Largest encoded message the host may send to one device.
    pub max_down: usize,
    /// Events waiting for an acknowledgement, in each direction, per device.
    pub max_pending: usize,
    /// Devices on the network at once (the TV included); local ones aside.
    pub max_devices: usize,
    /// Packets one link may send per second on average, and in one burst.
    /// Above, packets are dropped unread.
    pub packets_per_s: u64,
    pub packets_burst: u64,
    /// Events one device may have accepted per second, and in one burst.
    /// Above, events wait unacknowledged and the phone sends them again
    /// later: a flood slows down its sender, nobody else.
    pub events_per_s: u64,
    pub events_burst: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_up: 4 * 1024,
            max_down: 64 * 1024,
            max_pending: 256,
            max_devices: 24,
            packets_per_s: 240,
            packets_burst: 480,
            events_per_s: 30,
            events_burst: 60,
        }
    }
}

/// Why a device left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    /// Nothing heard for [`Timing::absent`]: Wi-Fi gone, phone locked.
    Silent,
    /// The transport closed (or the gamepad was unplugged).
    Closed,
    /// The player chose to leave.
    Quit,
}

/// Why the host turned a device away. The phone shows it and does not retry
/// on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The phone runs another version of the game or of salon: reload.
    Version,
    /// [`Limits::max_devices`] are already here.
    Full,
    /// The game takes no new devices now (it started, say).
    Closed,
    /// Claimed to be the TV without its key, or to be a local device.
    NotAllowed,
    /// The game removed this device.
    Removed,
}

/// How a device's link is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Present,
    /// Nothing heard for [`Timing::silent`]; it may still come back.
    Weak,
    Absent,
}
