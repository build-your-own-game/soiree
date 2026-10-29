//! Standalone diagnostic. No game, sockets, disk IO, or output in the timed loop.
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

type Net = ();
struct Clock;
impl Clock {
    fn instant() -> Instant { Instant::now() }
}

// PRODUCTION_WAIT

fn main() {
    let mode = std::env::args().nth(1).expect("diagnostic mode");
    let worker = std::thread::spawn(move || measure(&mode));
    worker.join().expect("diagnostic worker");
}

fn measure(mode: &str) {
    let (_sender, inbox) = sync_channel::<()>(16);
    let mutex = Mutex::new(());
    let condvar = Condvar::new();
    let start = Instant::now();
    let end = start + Duration::from_secs(3);
    let mut previous = start;
    let mut samples = Vec::with_capacity(200);
    let mut done = 0u128;
    let mut max_spin_observation_gap_ns = 0u128;
    let mut spin_observed = start;
    loop {
        // Same integer-ms 60 Hz deadlines and missed-slot policy as Steps.
        let due_ms = ((done + 1) * 1000).div_ceil(60);
        let deadline = start + Duration::from_millis(u64::try_from(due_ms).unwrap());
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match mode {
                "room-wait" => {
                    assert!(matches!(wait(&inbox, deadline), Err(RecvTimeoutError::Timeout)));
                }
                "recv-timeout" => {
                    assert!(matches!(inbox.recv_timeout(remaining), Err(RecvTimeoutError::Timeout)));
                }
                "sleep" => std::thread::sleep(remaining),
                "park" => std::thread::park_timeout(remaining),
                "condvar" => { drop(condvar.wait_timeout(mutex.lock().unwrap(), remaining).unwrap()); }
                "yield" => std::thread::yield_now(),
                "spin" => {
                    let observed = Instant::now();
                    max_spin_observation_gap_ns = max_spin_observation_gap_ns.max(observed.duration_since(spin_observed).as_nanos());
                    spin_observed = observed;
                    std::hint::spin_loop();
                }
                _ => panic!("unknown diagnostic mode"),
            }
        }
        let now = Instant::now();
        samples.push((now.duration_since(previous).as_nanos(), now.duration_since(deadline).as_nanos()));
        done = now.duration_since(start).as_millis() * 60 / 1000;
        previous = now;
        if now >= end { break; }
    }
    print!("{{\"mode\":\"{mode}\",\"elapsed_ns\":{},\"max_spin_observation_gap_ns\":{max_spin_observation_gap_ns},\"samples_gap_lateness_ns\":[", start.elapsed().as_nanos());
    for (index, (gap, late)) in samples.iter().enumerate() {
        if index != 0 { print!(","); }
        print!("[{gap},{late}]");
    }
    println!("]}}");
}
