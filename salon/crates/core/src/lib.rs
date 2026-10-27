//! # Salon: how the TV, the phones and the gamepads reach the game
//!
//! Every game of the evening runs on one computer plugged into the TV. Salon
//! connects that **host** to its **devices**: the TV page, phones (in a
//! browser or in the remote app), the host's own gamepads and keyboard. The
//! game never sees a socket, a token or a reconnection: it sees devices that
//! arrive, leave, come back, send events and hold states.
//!
//! This crate is the whole protocol, without I/O: no clock, no socket, no
//! thread (`#![no_std]`). The shells around it (`salon-host` on the
//! computer, `salon-web` in the pages) turn the world into calls and carry
//! out what it returns. Because it is a function of its inputs, the same code
//! runs in tests against a simulated network that loses, delays, repeats and
//! cuts, from a seed.
//!
//! ## Four ideas
//!
//! - **A device** ([`DeviceId`]) is one phone, one app, the TV, one gamepad,
//!   the keyboard. It keeps its identity while its connections come and go.
//!   Salon does not know players: the game decides that a device is a
//!   player, a team's buzzer, the second gamepad, or nothing.
//! - **An event** is something that happened and must count once: an
//!   answer, a vote, a stroke. Delivered once, in order, even across
//!   reconnections. [`Host::send`], [`Client::send`].
//! - **A state** is what is true now, and only the latest one matters: a
//!   gamepad's sticks, what a phone should show. A newer one replaces an
//!   older one, even half sent; it is resent until it arrives, and again on
//!   each reconnection. [`Host::show`], [`Client::set_state`].
//! - **Time** enters only through the calls, as [`Ms`].
//!
//! ## The host
//!
//! ```text
//! let mut host = Host::<Up, Down>::new(now, HostConfig { game, epoch, tv_key, … });
//! // The shell, for each transport:  host.opened(now, link, caps)
//! //                                 host.received(now, link, &bytes)
//! //                                 host.closed(now, link)
//! // Then, and at host.next_deadline():
//! while let Some(output) = host.poll(now) {
//!     match output {
//!         Output::Transmit { link, bytes } => /* send on that link */,
//!         Output::Close { link } => /* close it */,
//!         Output::Device(event) => game.on(event, &mut host),
//!     }
//! }
//! ```
//!
//! The game reacts to [`DeviceEvent`]s, and talks back with
//! [`Host::show`] (what a device should display now) and [`Host::send`]
//! (something that happened). On [`DeviceEvent::Arrived`], show the device
//! what it should see. On [`DeviceEvent::Left`], decide what an absence means
//! (pause, skip the turn, keep the seat): salon never frees a seat on its
//! own. Gamepads and the keyboard join through [`Host::add_local`].
//!
//! ## A device
//!
//! The phone page, the app and the TV run a [`Client`] the same way: the
//! shell opens a transport on [`ClientOutput::Connect`], reports
//! [`Client::opened`], [`Client::received`] and [`Client::closed`], calls
//! [`Client::wake`] when the page comes back to the foreground, and carries
//! out [`Client::poll`]. The page shows [`ClientOutput::State`], reacts to
//! [`ClientOutput::Event`], and tells the player about [`Status`].
//!
//! The device draws its own [`Token`] once, from the OS's secure random
//! generator, and keeps it (`localStorage`, the app's keychain). If storage
//! is broken, it draws a new one: it simply arrives as a new device. The TV
//! also sends the [`TvKey`] the host put in the TV page it served to its own
//! computer, and only a link from that computer ([`Caps::local`]) may be the
//! TV: a key replayed from the network is refused.
//!
//! ## What salon guarantees, so no game has to
//!
//! Each line names the item of `CERTIFICATION.md` it covers.
//!
//! - **Joining** is one `Hello`, answered by a `Welcome`; repeated until
//!   answered, and answering twice is harmless. A full room, a closed room,
//!   another version or a false TV get a typed [`Refusal`], never a silent
//!   lobby (C8).
//! - **One connection per device.** When a second tab or a reconnecting
//!   phone shows the same token, the newest wins; the old one is told it was
//!   replaced and does not reconnect on its own, so two tabs never evict each
//!   other in a loop (C5). A late close of the old connection does not touch
//!   the new one. The game hears nothing: the device never left.
//! - **Coming back.** After a cut, a reload or a locked phone, the device
//!   reconnects on its own, from 250 ms to 2 s between attempts and without
//!   end, and gets its identity, its latest state and every event it missed
//!   (C2, C4). The game gets [`DeviceEvent::Back`].
//! - **Silence is detected on both sides**: each side sends at least every
//!   [`Timing::heartbeat`]. After [`Timing::absent`] without a packet, the
//!   host sees [`DeviceEvent::Left`] with [`Why::Silent`], and the page drops
//!   its socket and reconnects without waiting for an `onclose` that iOS may
//!   never send (C3).
//! - **A restarting host** changes its [`Epoch`]. Devices reconnect on their
//!   own and get [`ClientOutput::Reset`]: they forget what the old run said
//!   and wait for the new one. With [`Host::known`] saved and given back in
//!   [`HostConfig::known`], devices come back as themselves; identities never
//!   go back to an old number (C7). Since the game of the new run never heard
//!   of them, each one's first connection is a [`DeviceEvent::Arrived`],
//!   under its old [`DeviceId`]. [`Host::shutdown`] tells every device first.
//! - **Hostile or broken input** never reaches the game: unreadable bytes,
//!   cut packets, nonsense fragments, wrong types are counted
//!   ([`DeviceInfo::garbage`]) and ignored (E1). A packet over 64 KiB closes
//!   that link only (E2). Packets past [`Limits::packets_per_s`] are dropped
//!   unread, and events past [`Limits::events_per_s`] wait unacknowledged:
//!   a flood slows down its sender and nobody else (E3). Without the token,
//!   nobody takes another's place; without the key, nobody becomes the TV
//!   (E4).
//! - **Nothing grows without bound**: every queue has a limit ([`Limits`]),
//!   and reaching it is an error returned to the caller
//!   ([`SendError::Full`]), never a silent drop.
//! - **Stale messages are the game's call, made easy**: every event carries
//!   the phase its sender had seen ([`Host::set_phase`]), so a late answer
//!   for a finished round is recognised and refused (E5), while
//!   simultaneous answers to the same question all count.
//!
//! What salon does **not** do: decide who plays, keep a seat, judge whether
//! an event is valid for the game, simulate anything, render anything.
//!
//! ## On the wire
//!
//! Salon needs only a pipe for packets, which may lose, repeat or reorder
//! them. One packet per send carries everything due: acknowledgements, the
//! phase, fragments of events, fragments of the state, and the times that
//! measure the round trip ([`DeviceInfo::rtt`], [`Client::host_time`]).
//! Events are numbered per device and resent from the oldest unacknowledged
//! until acknowledged, like Quake 3's reliable commands; messages bigger
//! than a packet are cut to fit the link ([`Caps`]).
//!
//! The transports, in the shells:
//!
//! - **WebRTC**, for phone pages: one DataChannel, unordered, without
//!   retransmission ([`Caps::DATAGRAM`]), to the host in ICE-lite. It works
//!   from a plain `http://` page on the local network, without any internet
//!   server: measured on 27/10/2026 with Safari on iOS 26.5, open in 90 ms,
//!   500 pings out of 500, round trip 4 ms median, 67 ms at the 99th
//!   percentile (`salon/LEARNINGS.md`).
//! - **WebSocket**, when WebRTC fails, and always for the TV, which is on the
//!   host's computer ([`Caps::STREAM`]).
//! - **WebSocket and UDP** for the remote app.
//!
//! The defaults of [`Timing`] and [`Limits`], and where each comes from, are
//! in `salon/LEARNINGS.md`.

#![no_std]

extern crate alloc;

mod client;
mod host;
mod stream;
mod types;
mod wire;

pub use client::{Client, ClientConfig, ClientOutput, SendError as ClientSendError, Status};
pub use host::{DeviceEvent, DeviceInfo, Host, HostConfig, Known, Output, SendError};
pub use types::{
    Caps, DeviceId, Epoch, Kind, Limits, LinkId, Ms, Name, Presence, Refusal, Timing, Token, TvKey,
    Why,
};
pub use wire::PROTOCOL;
