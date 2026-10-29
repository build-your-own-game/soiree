// Reference at 08af13e; deliberately not the current production implementation.
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
