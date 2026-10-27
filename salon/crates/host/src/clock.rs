//! The one place the host reads the wall clock.

use std::time::{Duration, Instant};

use salon_core::Ms;

/// Milliseconds since the host started, for salon and the game.
#[derive(Clone, Copy)]
pub struct Clock {
    start: Instant,
}

impl Clock {
    #[expect(
        clippy::disallowed_methods,
        reason = "the shell owns the clock; salon and the game only ever receive it as `Ms`"
    )]
    pub fn start() -> Clock {
        Clock {
            start: Instant::now(),
        }
    }

    pub fn now(self) -> Ms {
        Ms(u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    /// The moment `at` stands for.
    pub fn at(self, at: Ms) -> Instant {
        self.start + Duration::from_millis(at.0)
    }

    /// The `Instant` str0m wants for `now`.
    #[expect(
        clippy::disallowed_methods,
        reason = "str0m takes its time as an Instant; the shell owns the clock"
    )]
    pub fn instant() -> Instant {
        Instant::now()
    }
}
