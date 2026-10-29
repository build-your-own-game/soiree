//! Bounded FIFO with deadline and message wakeups. No helper thread or polling.
use std::io;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Instant;

#[cfg(target_os = "macos")]
mod macos;

pub(crate) struct Sender<T> {
    // Drop the actual sender before waking: the receiver must already be able
    // to observe disconnection when the last sender's notification is consumed.
    queue: Option<mpsc::SyncSender<T>>,
    #[cfg(target_os = "macos")]
    wake: std::sync::Arc<macos::Wake>,
}

pub(crate) struct Receiver<T> {
    queue: mpsc::Receiver<T>,
    #[cfg(target_os = "macos")]
    wake: std::sync::Arc<macos::Wake>,
}

#[derive(Debug)]
pub(crate) enum ReceiveError {
    Timeout,
    Disconnected,
    #[cfg(any(target_os = "macos", test))]
    Wait(io::Error),
}

pub(crate) fn channel<T>(capacity: usize) -> io::Result<(Sender<T>, Receiver<T>)> {
    // Rendezvous senders notify only after a receive; they cannot wake a
    // receiver parked in kqueue. The room always uses a positive bound.
    if capacity == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "room inbox needs positive capacity",
        ));
    }
    #[cfg(target_os = "macos")]
    let wake = std::sync::Arc::new(macos::Wake::new()?);
    let (sender, receiver) = mpsc::sync_channel(capacity);
    Ok((
        Sender {
            queue: Some(sender),
            #[cfg(target_os = "macos")]
            wake: std::sync::Arc::clone(&wake),
        },
        Receiver {
            queue: receiver,
            #[cfg(target_os = "macos")]
            wake,
        },
    ))
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            #[cfg(target_os = "macos")]
            wake: std::sync::Arc::clone(&self.wake),
        }
    }
}

impl<T> Sender<T> {
    /// A notification failure can follow enqueueing; it is returned and logged,
    /// never reported as a successful immediate wake. The native deadline
    /// remains armed independently of message notifications.
    pub(crate) fn send(&self, value: T) -> io::Result<()> {
        let Some(queue) = &self.queue else {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        };
        queue
            .send(value)
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        self.notify()
    }

    pub(crate) fn try_send(&self, value: T) -> io::Result<()> {
        let Some(queue) = &self.queue else {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        };
        queue.try_send(value).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => io::Error::from(io::ErrorKind::WouldBlock),
            mpsc::TrySendError::Disconnected(_) => io::Error::from(io::ErrorKind::BrokenPipe),
        })?;
        self.notify()
    }

    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            clippy::unused_self,
            clippy::unnecessary_wraps,
            reason = "the same interface is fallible and uses self for native macOS notifications"
        )
    )]
    fn notify(&self) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        self.wake.notify().inspect_err(|error| {
            // A timer still bounds the next wait. Never silently claim that
            // an unsuccessful native message notification was immediate.
            eprintln!("salon: room notification failed: {error}");
        })?;
        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        drop(self.queue.take());
        let _: io::Result<()> = self.notify();
    }
}

impl<T> Receiver<T> {
    pub(crate) fn recv_until(&self, deadline: Instant) -> Result<T, ReceiveError> {
        self.receive_with(deadline, || {})
    }

    fn receive_with(
        &self,
        deadline: Instant,
        mut before_wait: impl FnMut(),
    ) -> Result<T, ReceiveError> {
        loop {
            match self.queue.try_recv() {
                Ok(value) => return Ok(value),
                Err(TryRecvError::Disconnected) => return Err(ReceiveError::Disconnected),
                Err(TryRecvError::Empty) => {}
            }
            let remaining = deadline.saturating_duration_since(crate::clock::Clock::instant());
            if remaining.is_zero() {
                return Err(ReceiveError::Timeout);
            }
            before_wait();
            #[cfg(target_os = "macos")]
            self.wake.wait_until(deadline).map_err(ReceiveError::Wait)?;
            #[cfg(not(target_os = "macos"))]
            match self.queue.recv_timeout(remaining) {
                Ok(value) => return Ok(value),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ReceiveError::Disconnected);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    pub(crate) fn try_iter(&self) -> mpsc::TryIter<'_, T> {
        self.queue.try_iter()
    }

    #[cfg(test)]
    pub(crate) fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, ReceiveError> {
        let deadline = crate::clock::Clock::instant()
            .checked_add(timeout)
            .ok_or_else(|| {
                ReceiveError::Wait(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "deadline overflow",
                ))
            })?;
        self.recv_until(deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bounded_fifo_and_disconnection_preserve_queued_values() {
        assert!(
            matches!(channel::<()>(0), Err(error) if error.kind() == io::ErrorKind::InvalidInput)
        );
        let (sender, receiver) = channel(2).unwrap();
        sender.try_send(1).unwrap();
        sender.try_send(2).unwrap();
        assert_eq!(
            sender.try_send(3).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let other = sender.clone();
        drop(sender);
        drop(other);
        assert_eq!(receiver.recv_timeout(Duration::ZERO).unwrap(), 1);
        assert_eq!(receiver.recv_timeout(Duration::ZERO).unwrap(), 2);
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO),
            Err(ReceiveError::Disconnected)
        ));
    }

    #[test]
    fn receiver_drop_releases_a_blocked_sender() {
        let (sender, receiver) = channel(1).unwrap();
        sender.send(1).unwrap();
        let (started, ready) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            sender.send(2)
        });
        ready.recv().unwrap();
        drop(receiver);
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn concurrent_senders_preserve_every_value_and_per_sender_order() {
        let (sender, receiver) = channel(8).unwrap();
        std::thread::scope(|scope| {
            for producer in 0..4 {
                let sender = sender.clone();
                scope.spawn(move || {
                    for sequence in 0..100 {
                        sender.send((producer, sequence)).unwrap();
                    }
                });
            }
            drop(sender);
            let mut next = [0; 4];
            for _ in 0..400 {
                let (producer, sequence) = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
                assert_eq!(sequence, next[producer]);
                next[producer] += 1;
            }
            assert_eq!(next, [100; 4]);
            assert!(matches!(
                receiver.recv_timeout(Duration::from_secs(5)),
                Err(ReceiveError::Disconnected)
            ));
        });
    }

    #[test]
    fn message_between_empty_check_and_wait_is_not_lost() {
        let (sender, receiver) = channel(2).unwrap();
        let mut sender = Some(sender);
        let start = crate::clock::Clock::instant();
        let result = receiver.receive_with(start + Duration::from_secs(5), || {
            if let Some(sender) = sender.take() {
                sender.send(7).unwrap();
                sender.send(8).unwrap();
            }
        });
        assert_eq!(result.unwrap(), 7);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "wake lost until the deadline"
        );
        assert_eq!(receiver.recv_timeout(Duration::ZERO).unwrap(), 8);
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO),
            Err(ReceiveError::Disconnected)
        ));
    }

    #[test]
    fn last_sender_drop_between_empty_check_and_wait_wakes_disconnect() {
        let (sender, receiver) = channel::<()>(1).unwrap();
        let mut sender = Some(sender);
        let start = crate::clock::Clock::instant();
        let result = receiver.receive_with(start + Duration::from_secs(5), || drop(sender.take()));
        assert!(matches!(result, Err(ReceiveError::Disconnected)));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "disconnect wake lost"
        );
    }

    #[test]
    fn stale_notification_does_not_end_a_future_wait_early() {
        let (sender, receiver) = channel(1).unwrap();
        sender.send(1).unwrap();
        assert_eq!(receiver.recv_timeout(Duration::ZERO).unwrap(), 1);
        let start = crate::clock::Clock::instant();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(30)),
            Err(ReceiveError::Timeout)
        ));
        assert!(start.elapsed() >= Duration::from_millis(30));
        drop(sender);
    }
}
