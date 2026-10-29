//! Safe kqueue wrapper. USER is latched until consumed; TIMER is one-shot and
//! critical so ordinary timer coalescing does not absorb the room's deadlines.
use std::io;
use std::time::Instant;

use nix::errno::Errno;
use nix::sys::event::{EvFlags, EventFilter, FilterFlag, KEvent, Kqueue};

const USER: usize = 1;
const TIMER: usize = 2;

pub(super) struct Wake {
    queue: Kqueue,
}

impl Wake {
    pub(super) fn new() -> io::Result<Self> {
        let wake = Self {
            queue: Kqueue::new()?,
        };
        let user = KEvent::new(
            USER,
            EventFilter::EVFILT_USER,
            EvFlags::EV_ADD | EvFlags::EV_CLEAR,
            FilterFlag::empty(),
            0,
            0,
        );
        wake.change(user)?;
        Ok(wake)
    }

    fn change(&self, event: KEvent) -> io::Result<()> {
        loop {
            match self.queue.kevent(&[event], &mut [], None) {
                Ok(_) => return Ok(()),
                Err(Errno::EINTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn notify(&self) -> io::Result<()> {
        self.change(KEvent::new(
            USER,
            EventFilter::EVFILT_USER,
            EvFlags::empty(),
            FilterFlag::NOTE_TRIGGER,
            0,
            0,
        ))
    }

    pub(super) fn wait_until(&self, deadline: Instant) -> io::Result<()> {
        loop {
            let remaining = deadline.saturating_duration_since(crate::clock::Clock::instant());
            if remaining.is_zero() {
                return Ok(());
            }
            let nanos = isize::try_from(remaining.as_nanos()).unwrap_or(isize::MAX);
            // Nix preserves unlisted filter bits. Use libc's public SDK
            // constant, never a handwritten numeric value. Do not call nix
            // 0.30's fflags() on the returned event: it rejects unknown bits.
            let flags =
                FilterFlag::NOTE_NSECONDS | FilterFlag::from_bits_retain(nix::libc::NOTE_CRITICAL);
            let timer = KEvent::new(
                TIMER,
                EventFilter::EVFILT_TIMER,
                EvFlags::EV_ADD | EvFlags::EV_ONESHOT,
                flags,
                nanos,
                0,
            );
            let empty = KEvent::new(
                USER,
                EventFilter::EVFILT_USER,
                EvFlags::empty(),
                FilterFlag::empty(),
                0,
                0,
            );
            let mut events = [empty; 2];
            match self.queue.kevent(&[timer], &mut events, None) {
                Ok(count) => {
                    for event in events.iter().take(count) {
                        if event.flags().contains(EvFlags::EV_ERROR) {
                            let errno = i32::try_from(event.data())
                                .map_err(|_| io::Error::other("invalid kqueue errno"))?;
                            return Err(io::Error::from_raw_os_error(errno));
                        }
                        match (event.ident(), event.filter()) {
                            (USER, Ok(EventFilter::EVFILT_USER))
                            | (TIMER, Ok(EventFilter::EVFILT_TIMER)) => {}
                            _ => return Err(io::Error::other("unexpected room wake event")),
                        }
                    }
                    // The owner rechecks the queue and monotone deadline.
                    // A stale user/timer event is never itself a timeout.
                    return Ok(());
                }
                Err(Errno::EINTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}
