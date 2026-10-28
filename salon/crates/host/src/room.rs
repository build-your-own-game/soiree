//! The room thread: it alone owns the `Host` and the game. Transports talk
//! to it through bounded channels, and it never waits on them: a full
//! channel drops the packet, which salon resends.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::time::Duration;

use salon_core::{Caps, HostConfig, LinkId};

use crate::driver::{Driver, Effect, Input};

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
    Diagnostic(salon_core::journal::Diagnostic),
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
        let _: bool = match self {
            Outlet::Ws(outbox) => outbox.try_send(WsOut::Packet(bytes)).is_ok(),
            Outlet::Rtc(outbox) => outbox.try_send(RtcOut::Packet(link, bytes)).is_ok(),
        };
    }

    fn close(&self, link: LinkId) {
        // Full: the transport closes anyway when the room forgets the link.
        let _: bool = match self {
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
    game: G,
    config: HostConfig,
    tick: Duration,
    mut store: Store,
    clock: Clock,
) {
    let tick = u64::try_from(tick.as_millis()).unwrap_or(100).max(1);
    let mut driver = Driver::new(clock.now(), config, tick, game);
    let mut outlets: BTreeMap<LinkId, Outlet> = BTreeMap::new();
    store.save_epoch();
    let mut next_save = clock.now().plus(SAVE_EVERY);
    loop {
        let soon = Clock::instant();
        let soon = soon.checked_add(MAX_WAIT).unwrap_or(soon);
        let until = clock
            .at(driver.next_deadline())
            .map_or(soon, |wake| wake.min(soon));
        let mut inputs = Vec::new();
        match wait(inbox, until) {
            Ok(net) => {
                handle(&mut inputs, &mut outlets, net);
                for net in inbox.try_iter().take(INBOX) {
                    handle(&mut inputs, &mut outlets, net);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = clock.now();
        for effect in driver.step(now, inputs) {
            match effect {
                Effect::Transmit { link, bytes } => {
                    if let Some(outlet) = outlets.get(&link) {
                        outlet.send(link, bytes);
                    }
                }
                Effect::Close { link } => {
                    if let Some(outlet) = outlets.remove(&link) {
                        outlet.close(link);
                    }
                }
            }
        }
        if now >= next_save {
            store.save(driver.host().known());
            next_save = now.plus(SAVE_EVERY);
        }
    }
}

/// `SALON_LOG=1` prints every link that opens or closes, with its transport.
fn logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SALON_LOG").is_some())
}

fn handle(inputs: &mut Vec<Input>, outlets: &mut BTreeMap<LinkId, Outlet>, net: Net) {
    match net {
        Net::Diagnostic(report) => inputs.push(Input::Diagnostic { report }),
        Net::Opened { link, caps, outlet } => {
            if logging() {
                let transport = if matches!(outlet, Outlet::Rtc(_)) {
                    "WebRTC"
                } else {
                    "WebSocket"
                };
                eprintln!(
                    "salon: link {} opened ({transport}, {} bytes)",
                    link.0, caps.max_packet
                );
            }
            outlets.insert(link, outlet);
            inputs.push(Input::Opened { link, caps });
        }
        Net::Received { link, bytes } => inputs.push(Input::Received { link, bytes }),
        Net::Closed { link } => {
            if logging() {
                eprintln!("salon: link {} closed", link.0);
            }
            outlets.remove(&link);
            inputs.push(Input::Closed { link });
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]
mod tests {
    use std::sync::mpsc::sync_channel;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use salon_core::{DeviceEvent, Epoch, Host, Limits, Ms, Timing, TvKey};

    use super::*;

    struct StepSample {
        at: Duration,
        due: Duration,
    }

    /// Asks for a step every 1/60 s, like Sidequest, and records distinct
    /// wakes. Missed slots are skipped instead of creating catch-up samples
    /// one millisecond apart after a late wake.
    struct Steps {
        start: Clock,
        done: u64,
        seen: Arc<Mutex<Vec<StepSample>>>,
    }

    impl Game for Steps {
        type Up = ();
        type Down = ();

        fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}

        fn tick(&mut self, now: Ms, _: &mut Host<(), ()>) {
            if let Some(due) = self.next_deadline().filter(|&due| due <= now) {
                self.done = now.0.saturating_mul(60) / 1000;
                if let Ok(mut seen) = self.seen.lock() {
                    let start = self.start.at(Ms(0)).expect("the start itself");
                    seen.push(StepSample {
                        at: Clock::instant().duration_since(start),
                        due: Duration::from_millis(due.0),
                    });
                }
            }
        }

        fn next_deadline(&self) -> Option<Ms> {
            Some(Ms((self.done + 1).saturating_mul(1000).div_ceil(60)))
        }
    }

    #[test]
    fn cadence_probe_skips_missed_slots_instead_of_measuring_catch_up() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut game = Steps {
            start: Clock::start(),
            done: 0,
            seen: Arc::clone(&seen),
        };
        let config = HostConfig {
            game: 1,
            epoch: Epoch(1),
            tv_key: TvKey([0; 16]),
            timing: Timing::default(),
            limits: Limits::default(),
            known: Vec::new(),
        };
        let mut host = Host::new(Ms(0), config);
        game.tick(Ms(5_051), &mut host);
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(game.next_deadline(), Some(Ms(5_067)));
        game.tick(Ms(5_060), &mut host);
        assert_eq!(seen.lock().unwrap().len(), 1);
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
            .map(|pair| pair[1].at.saturating_sub(pair[0].at).as_micros())
            .collect();
        if gaps.is_empty() {
            println!("{} wakes: too few to measure a cadence", seen.len());
            return;
        }
        gaps.sort_unstable();
        let at = |per_mille: usize| gaps[(gaps.len() - 1) * per_mille / 1000];
        let mut lateness: Vec<u128> = seen
            .iter()
            .map(|sample| sample.at.saturating_sub(sample.due).as_micros())
            .collect();
        lateness.sort_unstable();
        let late = |per_mille: usize| lateness[(lateness.len() - 1) * per_mille / 1000];
        println!(
            "{} wakes, gaps µs: p50 {} p95 {} p99 {} max {}; deadline lateness µs: p50 {} p95 {} p99 {} max {}",
            seen.len(),
            at(500),
            at(950),
            at(990),
            at(1000),
            late(500),
            late(950),
            late(990),
            late(1000)
        );
    }
}
