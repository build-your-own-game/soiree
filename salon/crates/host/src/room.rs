//! The room thread: it alone owns the `Host` and the game. Transports talk
//! to it through bounded channels, and it never waits on them: a full
//! channel drops the packet, which salon resends.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::time::Duration;

use salon_core::{Caps, Host, HostConfig, LinkId, Ms, Output};

use crate::Game;
use crate::clock::Clock;
use crate::store::Store;

/// Network events waiting for the room, at most.
pub const INBOX: usize = 4096;
/// Packets waiting for one transport, at most.
pub const OUTBOX: usize = 256;
/// Identities are saved at most this often.
const SAVE_EVERY: u64 = 1_000;

pub enum Net {
    Opened {
        link: LinkId,
        caps: Caps,
        outlet: Outlet,
    },
    Received {
        link: LinkId,
        bytes: Vec<u8>,
    },
    Closed {
        link: LinkId,
    },
}

/// Where a link's packets go.
pub enum Outlet {
    Ws(SyncSender<WsOut>),
    Rtc(SyncSender<RtcOut>),
}

pub enum WsOut {
    Packet(Vec<u8>),
    Close,
}

pub enum RtcOut {
    Packet(LinkId, Vec<u8>),
    Close(LinkId),
}

impl Outlet {
    fn send(&self, link: LinkId, bytes: Vec<u8>) {
        // Full means the transport is behind: this packet is lost, like on
        // any network, and salon sends what matters again.
        let _ = match self {
            Outlet::Ws(outbox) => outbox.try_send(WsOut::Packet(bytes)).is_ok(),
            Outlet::Rtc(outbox) => outbox.try_send(RtcOut::Packet(link, bytes)).is_ok(),
        };
    }

    fn close(&self, link: LinkId) {
        let _ = match self {
            Outlet::Ws(outbox) => outbox.try_send(WsOut::Close).is_ok(),
            Outlet::Rtc(outbox) => outbox.try_send(RtcOut::Close(link)).is_ok(),
        };
    }
}

/// The longest the room sleeps without checking anything.
const MAX_WAIT: Duration = Duration::from_millis(50);
/// The last stretch before a deadline is waited for without a timer: macOS
/// coalesces the timers of programs without a window, and a 60 Hz game saw
/// steps 21 to 28 ms apart. Yielding costs this much CPU per deadline.
const SPIN: Duration = Duration::from_millis(2);

/// The next network event, or a timeout at `deadline`, on time.
fn wait(inbox: &Receiver<Net>, deadline: std::time::Instant) -> Result<Net, RecvTimeoutError> {
    let remaining = deadline.saturating_duration_since(Clock::instant());
    if let Some(sleep) = remaining.checked_sub(SPIN).filter(|sleep| !sleep.is_zero()) {
        match inbox.recv_timeout(sleep) {
            Err(RecvTimeoutError::Timeout) => {}
            other => return other,
        }
    }
    loop {
        match inbox.try_recv() {
            Ok(net) => return Ok(net),
            Err(TryRecvError::Disconnected) => return Err(RecvTimeoutError::Disconnected),
            Err(TryRecvError::Empty) if Clock::instant() >= deadline => {
                return Err(RecvTimeoutError::Timeout);
            }
            Err(TryRecvError::Empty) => std::thread::yield_now(),
        }
    }
}

pub fn run<G: Game>(
    inbox: &Receiver<Net>,
    mut game: G,
    config: HostConfig,
    tick: Duration,
    mut store: Store,
    clock: Clock,
) {
    let mut host = Host::<G::Up, G::Down>::new(clock.now(), config);
    let mut outlets: BTreeMap<LinkId, Outlet> = BTreeMap::new();
    let tick = u64::try_from(tick.as_millis()).unwrap_or(100).max(1);
    store.save_epoch();
    game.start(clock.now(), &mut host);
    let mut next_tick = clock.now();
    let mut next_save = clock.now().plus(SAVE_EVERY);
    loop {
        let wake = [host.next_deadline(), game.next_deadline(), Some(next_tick)]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(next_tick);
        match wait(inbox, clock.at(wake).min(Clock::instant() + MAX_WAIT)) {
            Ok(net) => {
                handle(&mut host, &mut outlets, clock.now(), net);
                // Take what else is waiting, without letting one flood
                // starve the timers.
                for net in inbox.try_iter().take(INBOX) {
                    handle(&mut host, &mut outlets, clock.now(), net);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = clock.now();
        if now >= next_tick || game.next_deadline().is_some_and(|at| at <= now) {
            game.tick(now, &mut host);
            next_tick = next_tick.plus(tick).max(now);
        }
        while let Some(output) = host.poll(now) {
            match output {
                Output::Transmit { link, bytes } => {
                    if let Some(outlet) = outlets.get(&link) {
                        outlet.send(link, bytes);
                    }
                }
                Output::Close { link } => {
                    if let Some(outlet) = outlets.remove(&link) {
                        outlet.close(link);
                    }
                }
                Output::Device(event) => game.on(now, event, &mut host),
            }
        }
        if now >= next_save {
            store.save(host.known());
            next_save = now.plus(SAVE_EVERY);
        }
    }
}

/// `SALON_LOG=1` prints every link that opens or closes, with its transport.
fn logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SALON_LOG").is_some())
}

fn handle<Up: serde::de::DeserializeOwned, Down: serde::Serialize>(
    host: &mut Host<Up, Down>,
    outlets: &mut BTreeMap<LinkId, Outlet>,
    now: Ms,
    net: Net,
) {
    match net {
        Net::Opened { link, caps, outlet } => {
            if logging() {
                let transport = if matches!(outlet, Outlet::Rtc(_)) {
                    "WebRTC"
                } else {
                    "WebSocket"
                };
                eprintln!(
                    "salon: link {} opened ({transport}, {} bytes) at {} ms",
                    link.0, caps.max_packet, now.0
                );
            }
            outlets.insert(link, outlet);
            host.opened(now, link, caps);
        }
        Net::Received { link, bytes } => host.received(now, link, &bytes),
        Net::Closed { link } => {
            if logging() {
                eprintln!("salon: link {} closed at {} ms", link.0, now.0);
            }
            outlets.remove(&link);
            host.closed(now, link);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::sync_channel;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use salon_core::{DeviceEvent, Epoch, Limits, Timing, TvKey};

    use super::*;

    /// Asks for a step every 1/60 s, like Sidequest, and records when each
    /// step ran.
    struct Steps {
        start: Clock,
        done: u64,
        seen: Arc<Mutex<Vec<std::time::Duration>>>,
    }

    impl Game for Steps {
        type Up = ();
        type Down = ();

        fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}

        fn tick(&mut self, now: Ms, _: &mut Host<(), ()>) {
            if self.next_deadline().is_some_and(|due| due <= now) {
                self.done += 1;
                if let Ok(mut seen) = self.seen.lock() {
                    seen.push(Clock::instant().duration_since(self.start.at(Ms(0))));
                }
            }
        }

        fn next_deadline(&self) -> Option<Ms> {
            Some(Ms((self.done + 1).saturating_mul(1000).div_ceil(60)))
        }
    }

    #[test]
    #[ignore = "a timing measurement: run it on an idle machine"]
    fn steps_come_on_time() {
        let clock = Clock::start();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let game = Steps {
            start: clock,
            done: 0,
            seen: Arc::clone(&seen),
        };
        let (net, inbox) = sync_channel(16);
        let config = HostConfig {
            game: 1,
            epoch: Epoch(1),
            tv_key: TvKey([0; 16]),
            timing: Timing::default(),
            limits: Limits::default(),
            known: Vec::new(),
        };
        let store = Store::open(None, 1, [0; 16]);
        let room =
            thread::spawn(move || run(&inbox, game, config, Duration::from_secs(1), store, clock));
        thread::sleep(Duration::from_secs(3));
        drop(net);
        room.join().unwrap();
        let seen = seen.lock().unwrap();
        let mut gaps: Vec<u128> = seen
            .windows(2)
            .map(|pair| pair[1].saturating_sub(pair[0]).as_micros())
            .collect();
        gaps.sort_unstable();
        let at = |per_mille: usize| gaps[(gaps.len() - 1) * per_mille / 1000];
        println!(
            "{} steps, gaps µs: p50 {} p99 {} max {}",
            seen.len(),
            at(500),
            at(990),
            at(1000)
        );
    }
}
