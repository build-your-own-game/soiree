//! Admission precedes thread creation. A permit covers the entire connection,
//! including its WebSocket writer; remote peers cannot consume local reserves.

use crate::clock::Clock;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Counts {
    total: usize,
    remote: usize,
}

pub struct Admission {
    counts: Mutex<Counts>,
    maximum: usize,
    remote_maximum: usize,
}

pub struct Permit {
    admission: Arc<Admission>,
    remote: bool,
}

impl Admission {
    /// Each device may use the native app door and a WebSocket fallback at
    /// once. Keep additional HTTP headroom and local organizer capacity.
    pub fn for_devices(devices: usize) -> Arc<Self> {
        Self::new(devices.saturating_mul(2).saturating_add(64).max(128), 32)
    }

    pub fn new(maximum: usize, local_reserve: usize) -> Arc<Self> {
        Arc::new(Self {
            counts: Mutex::new(Counts::default()),
            maximum,
            remote_maximum: maximum.saturating_sub(local_reserve),
        })
    }

    pub fn acquire(self: &Arc<Self>, peer: std::net::IpAddr) -> Option<Permit> {
        let remote = !peer.to_canonical().is_loopback();
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        if counts.total >= self.maximum || (remote && counts.remote >= self.remote_maximum) {
            return None;
        }
        counts.total = counts.total.saturating_add(1);
        if remote {
            counts.remote = counts.remote.saturating_add(1);
        }
        Some(Permit {
            admission: Arc::clone(self),
            remote,
        })
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut counts = self
            .admission
            .counts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        counts.total = counts.total.saturating_sub(1);
        if self.remote {
            counts.remote = counts.remote.saturating_sub(1);
        }
    }
}

/// Socket timeouts alone renew at every syscall, allowing slow-drip requests.
/// Recompute the remaining budget before every actual read instead.
pub struct DeadlineReader {
    stream: Arc<TcpStream>,
    deadline: Instant,
}

fn deadline(budget: Duration) -> Instant {
    let now = Clock::instant();
    now.checked_add(budget).unwrap_or(now)
}

impl DeadlineReader {
    pub fn new(stream: Arc<TcpStream>, budget: Duration) -> Self {
        Self {
            stream,
            deadline: deadline(budget),
        }
    }

    pub fn renew(&mut self, budget: Duration) {
        self.deadline = deadline(budget);
    }
}

impl Read for DeadlineReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Clock::instant());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection read deadline",
            ));
        }
        self.stream.set_read_timeout(Some(remaining))?;
        (&*self.stream).read(buffer)
    }
}

/// A complete response/frame shares one budget, even with partial writes.
pub struct DeadlineWriter<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}

impl<'a> DeadlineWriter<'a> {
    pub fn new(stream: &'a TcpStream, budget: Duration) -> Self {
        Self {
            stream,
            deadline: deadline(budget),
        }
    }
}

impl Write for DeadlineWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Clock::instant());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection write deadline",
            ));
        }
        self.stream.set_write_timeout(Some(remaining))?;
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{IpAddr, TcpListener};
    use std::thread;

    #[test]
    fn admission_is_bounded_and_reserves_local_capacity() {
        let pool = Admission::new(3, 1);
        let remote = IpAddr::from([192, 0, 2, 1]);
        let local = IpAddr::from([127, 0, 0, 1]);
        let a = pool.acquire(remote).unwrap();
        let b = pool.acquire(remote).unwrap();
        assert!(pool.acquire(remote).is_none());
        let c = pool.acquire(local).unwrap();
        assert!(pool.acquire(local).is_none());
        drop(a);
        let d = pool.acquire(remote).unwrap();
        drop((b, c, d));
        assert_eq!(pool.counts.lock().unwrap().total, 0);
    }

    #[test]
    fn the_largest_game_keeps_room_for_app_and_fallback_per_device() {
        let pool = Admission::for_devices(72);
        let permits: Vec<_> = (0..144)
            .map(|_| pool.acquire(IpAddr::from([192, 0, 2, 1])).unwrap())
            .collect();
        assert_eq!(permits.len(), 144);
        let mut extras = Vec::new();
        while let Some(permit) = pool.acquire(IpAddr::from([192, 0, 2, 2])) {
            extras.push(permit);
        }
        assert_eq!(extras.len(), 32);
        assert!(pool.acquire(IpAddr::from([127, 0, 0, 1])).is_some());
    }

    #[test]
    fn slow_drip_does_not_renew_the_request_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let writer = thread::spawn(move || {
            for _ in 0..50 {
                if client.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let started = Clock::instant();
        let mut reader = DeadlineReader::new(Arc::new(server), Duration::from_millis(80));
        let mut bytes = Vec::new();
        let error = reader.read_to_end(&mut bytes).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        assert_ne!(bytes, [] as [u8; 0]);
        assert!(started.elapsed() < Duration::from_millis(400));
        drop(reader);
        writer.join().unwrap();
    }

    #[test]
    fn a_slow_reader_does_not_renew_the_whole_write_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let reader = thread::spawn(move || {
            let mut buffer = [0; 1024];
            for _ in 0..50 {
                if client.read(&mut buffer).unwrap_or(0) == 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let body = vec![0; 32 * 1024 * 1024];
        let started = Clock::instant();
        let error = DeadlineWriter::new(&server, Duration::from_millis(80))
            .write_all(&body)
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        assert!(started.elapsed() < Duration::from_millis(400));
        drop(server);
        reader.join().unwrap();
    }

    #[test]
    fn successful_short_writes_do_not_reset_the_total_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let mut writer = DeadlineWriter::new(&server, Duration::from_millis(30));
        writer.write_all(b"a").unwrap();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(
            writer.write_all(b"b").unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        drop(server);
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        assert_eq!(received, b"a");
    }
}
