//! The bench of the Swift shell: nothing else runs `GameConnection`,
//! `RemoteModel` or `WebRemote`, and Rust's tests cannot see what they do.
//!
//! [`shell`] builds the real sources of `ios/Remote` into a small app, with
//! `app/BenchApp.swift` in place of `RemoteApp.swift`, and runs it on a
//! simulator. A test drives it line by line. [`host::FakeHost`] is the game it
//! joins, which cuts, freezes or fails on command and records every frame.
//! `salon-door/` is a real salon, for what only a real salon says.
//!
//! `cargo test` needs macOS, Xcode and an iPhone simulator. Without them it
//! fails: a bench that could not run has proved nothing.

#![expect(
    clippy::disallowed_methods,
    reason = "a bench lives on the wall clock: the app and the game do"
)]

pub mod host;
pub mod salon;
pub mod shell;

use std::time::{Duration, Instant};

/// Until `done` holds; false if `limit` passed first.
pub fn eventually(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    done()
}
