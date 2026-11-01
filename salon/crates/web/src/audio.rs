//! Short, owned TV cues, independent of networking and game rules.
//!
//! A page supplies increasing event identities and timestamps on one clock.
//! The first observation establishes a silent baseline. An unavailable,
//! hidden or muted output consumes events without queuing them: resuming
//! cannot play a backlog. A discontinuity (reconnection/room change) primes
//! a new silent baseline. The browser adapter owns a bounded audio graph.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::Audio;
#[cfg(all(target_arch = "wasm32", feature = "audio-test"))]
pub use browser::render_reference;

/// Maximum simultaneous oscillators. Every oscillator peaks at 0.12, so even
/// their arithmetic sum remains below full scale at maximum master volume.
pub const MAX_VOICES: usize = 4;
/// A cue older than this is no longer useful feedback.
pub const MAX_AGE_MS: f64 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Start,
    Advance,
    Result,
    End,
}

impl Cue {
    pub fn frequencies(self) -> (f32, f32) {
        match self {
            Self::Start => (440.0, 660.0),
            Self::Advance => (550.0, 740.0),
            Self::Result => (660.0, 990.0),
            Self::End => (880.0, 1320.0),
        }
    }
}

/// Pure admission policy; no wall clock, browser or hidden retries.
#[derive(Default, Debug)]
pub struct Gate {
    latest: Option<u64>,
}

impl Gate {
    pub fn discontinuity(&mut self) {
        self.latest = None;
    }

    /// Every identity is consumed even when stale/unavailable. Backwards and
    /// duplicate identities cannot replay an effect. A new sequence is only
    /// valid after an explicit discontinuity.
    pub fn observe(&mut self, sequence: u64, at_ms: f64, now_ms: f64) -> bool {
        let previous = self.latest;
        if previous.is_some_and(|latest| sequence <= latest) {
            return false;
        }
        self.latest = Some(sequence);
        previous.is_some()
            && at_ms.is_finite()
            && now_ms.is_finite()
            && at_ms <= now_ms
            && now_ms - at_ms <= MAX_AGE_MS
    }
}

/// Detect semantic transitions while ignoring clock, player count and redraws.
/// A consumer that reconnects must clear this detector as well as its output.
#[derive(Debug)]
pub struct Transitions<K> {
    previous: Option<K>,
    sequence: u64,
}

impl<K> Default for Transitions<K> {
    fn default() -> Self {
        Self {
            previous: None,
            sequence: 0,
        }
    }
}

impl<K: Eq> Transitions<K> {
    pub fn observe(&mut self, key: K) -> Option<u64> {
        if self.previous.as_ref() == Some(&key) {
            return None;
        }
        self.previous = Some(key);
        self.sequence = self.sequence.saturating_add(1);
        Some(self.sequence)
    }

    pub fn clear(&mut self) {
        self.previous = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_do_not_replay_after_duplicates_old_packets_or_discontinuities() {
        let mut gate = Gate::default();
        assert!(!gate.observe(1, 0.0, 0.0));
        assert!(gate.observe(2, 5.0, 10.0));
        assert!(!gate.observe(2, 10.0, 10.0));
        assert!(!gate.observe(1, 10.0, 10.0));
        assert!(!gate.observe(3, 10.0, 1000.0));
        assert!(!gate.observe(3, 1000.0, 1000.0));
        gate.discontinuity();
        assert!(!gate.observe(0, 1000.0, 1000.0));
        assert!(gate.observe(1, 1000.0, 1000.0));
    }

    #[test]
    fn invalid_timestamps_and_future_cues_are_consumed() {
        for timestamp in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 101.0] {
            let mut gate = Gate::default();
            assert!(!gate.observe(0, 0.0, 0.0));
            assert!(!gate.observe(1, timestamp, 100.0));
            assert!(!gate.observe(1, 100.0, 100.0));
        }
    }

    #[test]
    fn semantic_redraws_do_not_manufacture_events() {
        let mut transitions = Transitions::default();
        assert_eq!(transitions.observe("lobby"), Some(1));
        assert_eq!(transitions.observe("lobby"), None);
        assert_eq!(transitions.observe("question"), Some(2));
        transitions.clear();
        assert_eq!(transitions.observe("question"), Some(3));
    }
}
