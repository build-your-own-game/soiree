//! What a page remembers of WebRTC failing. Where UDP is blocked, every
//! attempt costs the whole negotiation window before WebSocket takes over: a
//! reload must not pay it again. The decision is here, without a browser; its
//! value lives in the tab's session storage.

use serde::{Deserialize, Serialize};

/// In session storage: one tab, gone with it. Another address of the host is
/// another origin, with its own memory.
pub(crate) const KEY: &str = "salon.rtc-failures.v1";
/// After this many WebRTC failures in a row, a page sticks to WebSocket.
const FAILURES: u32 = 2;
/// How long a failure stays true. A network change is announced by `online`
/// at best, and only when the connection was lost: iOS says nothing when it
/// moves to another Wi-Fi, or when the host opens its firewall. This bounds
/// how long such a phone stays on WebSocket, against a window paid again this
/// often by a phone where UDP stays blocked.
const REMEMBERED_MS: u64 = 10 * 60 * 1_000;

/// WebRTC's failures in a row, the last one at `at`, in wall-clock
/// milliseconds: unlike `performance.now()`, it means something to the next
/// page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RtcMemory {
    failures: u32,
    at: u64,
}

impl RtcMemory {
    /// What a new page starts from. One remembered failure is enough here:
    /// the second chance a page gives WebRTC is for a glitch during a game,
    /// and the wait of a page that is loading is the one players see.
    pub(crate) fn loaded(stored: Option<&str>, now: u64) -> RtcMemory {
        let memory: RtcMemory = stored
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        if memory.failures == 0 || memory.is_old(now) {
            return RtcMemory::default();
        }
        RtcMemory {
            failures: memory.failures.max(FAILURES),
            at: memory.at,
        }
    }

    pub(crate) fn failed(self, now: u64) -> RtcMemory {
        RtcMemory {
            failures: self.failures.saturating_add(1),
            at: now,
        }
    }

    pub(crate) fn use_rtc(self, now: u64) -> bool {
        self.failures < FAILURES || self.is_old(now)
    }

    fn is_old(self, now: u64) -> bool {
        now.checked_sub(self.at)
            .is_none_or(|age| age >= REMEMBERED_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVED: u64 = 1_000_000;

    /// What the next page reads after these failures.
    fn saved(failures: u32) -> String {
        let mut memory = RtcMemory::default();
        for _ in 0..failures {
            memory = memory.failed(SAVED);
        }
        serde_json::to_string(&memory).unwrap()
    }

    #[test]
    fn a_first_load_tries_webrtc() {
        assert!(RtcMemory::loaded(None, SAVED).use_rtc(SAVED));
    }

    #[test]
    fn a_page_gives_webrtc_a_second_chance_then_sticks_to_websocket() {
        let once = RtcMemory::default().failed(SAVED);
        assert!(once.use_rtc(SAVED));
        assert!(!once.failed(SAVED).use_rtc(SAVED));
    }

    /// The point: the reload after one failed window goes straight to
    /// WebSocket.
    #[test]
    fn a_reload_does_not_pay_the_window_again() {
        let now = SAVED + 1_000;
        assert!(!RtcMemory::loaded(Some(&saved(1)), now).use_rtc(now));
        assert!(!RtcMemory::loaded(Some(&saved(2)), now).use_rtc(now));
    }

    #[test]
    fn webrtc_is_tried_again_once_the_memory_is_old() {
        let now = SAVED + REMEMBERED_MS;
        assert!(RtcMemory::loaded(Some(&saved(1)), now).use_rtc(now));
        // A page that stayed open comes back to it too, at its next connection.
        let stuck = RtcMemory::default().failed(SAVED).failed(SAVED);
        assert!(!stuck.use_rtc(now - 1));
        assert!(stuck.use_rtc(now));
        // And failing again starts a new period.
        assert!(!stuck.failed(now).use_rtc(now + 1));
    }

    /// A clock that went back says nothing of the memory's age.
    #[test]
    fn a_clock_that_goes_back_does_not_keep_webrtc_away() {
        let stuck = RtcMemory::default().failed(SAVED).failed(SAVED);
        assert!(stuck.use_rtc(SAVED - 1));
    }

    /// `online`, or a WebRTC link that opens: the failures no longer say
    /// anything, on this page or on the next.
    #[test]
    fn a_forgotten_memory_tries_webrtc_at_once() {
        let stuck = RtcMemory::default().failed(SAVED).failed(SAVED);
        assert!(!stuck.use_rtc(SAVED));
        let forgotten = RtcMemory::default();
        assert!(forgotten.use_rtc(SAVED));
        let text = serde_json::to_string(&forgotten).unwrap();
        assert!(RtcMemory::loaded(Some(&text), SAVED).use_rtc(SAVED));
    }

    /// Principle 8: what comes from storage is validated, and broken storage
    /// must never keep WebRTC away.
    #[test]
    fn a_broken_memory_is_no_memory() {
        for text in ["", "{", "3", r#"{"failures":-1,"at":0}"#, r#"{"at":1}"#] {
            assert!(
                RtcMemory::loaded(Some(text), SAVED).use_rtc(SAVED),
                "{text}"
            );
        }
    }
}
