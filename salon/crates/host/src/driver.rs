//! The host's deterministic loop, used unchanged by the real room and tests.
//! The caller owns time and transports; the driver only sees ordered inputs.

use crate::Game;
use salon_core::{Caps, Host, HostConfig, LinkId, Ms, Output};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Diagnostic {
        report: salon_core::journal::Diagnostic,
    },
    Opened {
        link: LinkId,
        caps: Caps,
    },
    Received {
        link: LinkId,
        bytes: Vec<u8>,
    },
    Closed {
        link: LinkId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Transmit { link: LinkId, bytes: Vec<u8> },
    Close { link: LinkId },
}

pub struct Driver<G: Game> {
    host: Host<G::Up, G::Down>,
    game: G,
    tick: u64,
    next_tick: Ms,
    now: Ms,
    last_tick: Option<Ms>,
}

impl<G: Game> Driver<G> {
    pub fn new(now: Ms, config: HostConfig, tick_ms: u64, mut game: G) -> Self {
        let mut host = Host::new(now, config);
        game.start(now, &mut host);
        Self {
            host,
            game,
            tick: tick_ms.max(1),
            next_tick: now,
            now,
            last_tick: None,
        }
    }

    pub fn now(&self) -> Ms {
        self.now
    }

    pub fn game(&self) -> &G {
        &self.game
    }
    pub fn game_mut(&mut self) -> &mut G {
        &mut self.game
    }
    pub fn host(&self) -> &Host<G::Up, G::Down> {
        &self.host
    }

    pub fn next_deadline(&self) -> Ms {
        [
            self.host.next_deadline(),
            self.game.next_deadline(),
            Some(self.next_tick),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(self.next_tick)
    }

    /// A backwards clock is rejected without touching state. Network inputs
    /// are processed before due ticks, and device events after ticks, exactly
    /// as in the room loop. Preserve this ordering when changing this code.
    pub fn step(&mut self, now: Ms, inputs: impl IntoIterator<Item = Input>) -> Vec<Effect> {
        if now < self.now {
            return Vec::new();
        }
        self.now = now;
        for input in inputs {
            match input {
                Input::Diagnostic { report } => self.game.diagnostic(now, &report),
                Input::Opened { link, caps } => self.host.opened(now, link, caps),
                Input::Received { link, bytes } => self.host.received(now, link, &bytes),
                Input::Closed { link } => self.host.closed(now, link),
            }
        }
        if self.last_tick != Some(now)
            && (now >= self.next_tick || self.game.next_deadline().is_some_and(|due| due <= now))
        {
            self.game.tick(now, &mut self.host);
            self.last_tick = Some(now);
            if now >= self.next_tick {
                // Keep the periodic grid, skipping missed slots. Replaying
                // every missed tick at the same timestamp would duplicate
                // simulation work and create an immediate busy loop.
                let slots = now
                    .since(self.next_tick)
                    .checked_div(self.tick)
                    .unwrap_or(0)
                    .saturating_add(1);
                self.next_tick = self.next_tick.plus(self.tick.saturating_mul(slots));
            }
        }
        let mut effects = Vec::new();
        while let Some(output) = self.host.poll(now) {
            match output {
                Output::Transmit { link, bytes } => effects.push(Effect::Transmit { link, bytes }),
                Output::Close { link } => effects.push(Effect::Close { link }),
                Output::Device(event) => self.game.on(now, event, &mut self.host),
            }
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use salon_core::{DeviceEvent, Epoch, Limits, Timing, TvKey};
    #[derive(Default)]
    struct Ticks(Vec<Ms>);
    impl Game for Ticks {
        type Up = ();
        type Down = ();
        fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}
        fn tick(&mut self, now: Ms, _: &mut Host<(), ()>) {
            self.0.push(now);
        }
    }
    #[test]
    fn late_wakes_skip_missed_periodic_slots_without_double_ticking() {
        let config = HostConfig {
            game: 1,
            epoch: Epoch(1),
            tv_key: TvKey([0; 16]),
            timing: Timing::default(),
            limits: Limits::default(),
            known: Vec::new(),
        };
        let mut driver = Driver::new(Ms(0), config, 17, Ticks::default());
        let _: Vec<Effect> = driver.step(Ms(0), []);
        let _: Vec<Effect> = driver.step(Ms(1000), []);
        assert_eq!(driver.next_deadline(), Ms(1003));
        let _: Vec<Effect> = driver.step(Ms(1000), []);
        let _: Vec<Effect> = driver.step(Ms(999), []);
        assert_eq!(driver.game().0, vec![Ms(0), Ms(1000)]);
        let _: Vec<Effect> = driver.step(Ms(1003), []);
        assert_eq!(driver.game().0, vec![Ms(0), Ms(1000), Ms(1003)]);
    }
}
