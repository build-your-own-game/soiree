//! Reliability over a link that may lose, repeat or reorder packets, written
//! once for both sides.
//!
//! - **Events** are numbered and kept until acknowledged; every send carries
//!   the due ones from the oldest, so they arrive in order with no receive
//!   buffer (Quake 3's reliable commands, Fiedler's reliable messages). The
//!   numbers belong to the device, not to the link: what was in flight when
//!   a link died goes again on the next one, and is not delivered twice.
//! - **The state** is the latest value only: a newer one replaces an older
//!   one, even half sent. It goes again until acknowledged.
//!
//! A message bigger than one packet is cut into fragments, sized for the
//! current link when it is sent.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use crate::types::Ms;
use crate::wire::{EventPart, PACKET_OVERHEAD, PART_OVERHEAD, StatePart};

/// Fragments of one message being put back together.
type Slots = Vec<Option<Vec<u8>>>;

/// Most fragments one message may have; with the smallest packets, 64 KiB.
const MAX_FRAGMENTS: u16 = 1024;

/// Data bytes per fragment on a link with packets of `packet` bytes.
pub(crate) fn fragment_size(packet: usize) -> usize {
    packet
        .saturating_sub(PACKET_OVERHEAD + PART_OVERHEAD)
        .max(64)
}

fn fragment_count(len: usize, fragment: usize) -> u16 {
    let count = len.div_ceil(fragment).max(1);
    u16::try_from(count).unwrap_or(u16::MAX)
}

fn fragment(data: &[u8], index: u16, fragment: usize) -> Vec<u8> {
    let start = usize::from(index).saturating_mul(fragment);
    data.iter().skip(start).take(fragment).copied().collect()
}

/// Room left in a packet being filled.
pub(crate) struct Budget {
    pub bytes: usize,
}

impl Budget {
    pub fn for_packet(packet: usize) -> Budget {
        Budget {
            bytes: packet.saturating_sub(PACKET_OVERHEAD),
        }
    }

    fn take(&mut self, data: usize) -> bool {
        let cost = data + PART_OVERHEAD;
        if cost > self.bytes {
            return false;
        }
        self.bytes -= cost;
        true
    }
}

struct Outgoing {
    seq: u32,
    data: Vec<u8>,
    phase: u32,
    at: Option<u64>,
    /// When each fragment last left, for the fragment size of `cut`.
    sent: Vec<Option<Ms>>,
    cut: usize,
}

/// Events to send, kept until acknowledged.
pub(crate) struct OutEvents {
    next: u32,
    queue: VecDeque<Outgoing>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Full;

impl OutEvents {
    pub fn new(next: u32) -> Self {
        OutEvents {
            next,
            queue: VecDeque::new(),
        }
    }

    pub fn push(
        &mut self,
        data: Vec<u8>,
        phase: u32,
        at: Option<u64>,
        max: usize,
    ) -> Result<u32, Full> {
        if self.queue.len() >= max {
            return Err(Full);
        }
        let seq = self.next;
        self.next = self.next.checked_add(1).ok_or(Full)?;
        self.queue.push_back(Outgoing {
            seq,
            data,
            phase,
            at,
            sent: Vec::new(),
            cut: 0,
        });
        Ok(seq)
    }

    /// The other side has every event below `next`.
    pub fn ack(&mut self, next: u32) {
        while self.queue.front().is_some_and(|event| event.seq < next) {
            self.queue.pop_front();
        }
    }

    /// The oldest event still waiting (or the next number if none).
    pub fn oldest(&self) -> u32 {
        self.queue.front().map_or(self.next, |event| event.seq)
    }

    /// A new link: everything goes again, cut for it.
    pub fn unsend(&mut self) {
        for event in &mut self.queue {
            event.sent.clear();
            event.cut = 0;
        }
    }

    /// Keeps what waits but numbers it from `next` (the first welcome says
    /// where the host starts).
    pub fn renumber(&mut self, next: u32) {
        let mut seq = next;
        for event in &mut self.queue {
            event.seq = seq;
            seq = seq.saturating_add(1);
        }
        self.next = seq;
    }

    /// Forgets everything and numbers from `next` (the host restarted).
    pub fn restart(&mut self, next: u32) {
        self.queue.clear();
        self.next = next;
    }

    /// When something waiting goes again, if anything waits.
    pub fn due_at(&self, resend: u64) -> Option<Ms> {
        self.queue
            .iter()
            .flat_map(|event| {
                if event.sent.is_empty() {
                    return vec![Ms(0)];
                }
                event
                    .sent
                    .iter()
                    .map(|sent| sent.map_or(Ms(0), |at| at.plus(resend)))
                    .collect()
            })
            .min()
    }

    /// Fragments due now, from the oldest, that fit `budget`.
    pub fn take(
        &mut self,
        now: Ms,
        resend: u64,
        packet: usize,
        budget: &mut Budget,
    ) -> Vec<EventPart> {
        let cut = fragment_size(packet);
        let mut parts = Vec::new();
        for event in &mut self.queue {
            if event.cut != cut {
                event.cut = cut;
                event.sent = vec![None; usize::from(fragment_count(event.data.len(), cut))];
            }
            let count = fragment_count(event.data.len(), cut);
            for (index, sent) in (0..count).zip(event.sent.iter_mut()) {
                let due = sent.is_none_or(|at| now.since(at) >= resend);
                if !due {
                    continue;
                }
                let data = fragment(&event.data, index, cut);
                if !budget.take(data.len()) {
                    return parts;
                }
                *sent = Some(now);
                parts.push(EventPart {
                    seq: event.seq,
                    index,
                    count,
                    phase: event.phase,
                    at: event.at,
                    data,
                });
            }
        }
        parts
    }
}

/// An event put back together, in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Arrived {
    pub data: Vec<u8>,
    pub phase: u32,
    pub at: Option<u64>,
}

/// Events received, delivered once each and in order.
pub(crate) struct InEvents {
    next: u32,
    partial: Option<(u16, Slots)>,
}

/// What a batch of fragments gave.
pub(crate) struct Received {
    pub events: Vec<Arrived>,
    /// Fragments that made no sense (too big, wrong count): hostile or buggy.
    pub garbage: u32,
    /// Accepted anything new, so an acknowledgement is worth sending soon.
    pub progressed: bool,
}

impl InEvents {
    pub fn new(next: u32) -> Self {
        InEvents {
            next,
            partial: None,
        }
    }

    pub fn next(&self) -> u32 {
        self.next
    }

    /// The sender says it no longer has what is below `first`: skip ahead.
    pub fn skip_to(&mut self, first: u32) {
        if first > self.next {
            self.next = first;
            self.partial = None;
        }
    }

    pub fn restart(&mut self, next: u32) {
        self.next = next;
        self.partial = None;
    }

    /// Takes the fragments in order. `allow` says how many more whole events
    /// may be accepted now; past it, the rest waits for a resend.
    pub fn receive(
        &mut self,
        parts: Vec<EventPart>,
        max: usize,
        mut allow: impl FnMut() -> bool,
    ) -> Received {
        let mut received = Received {
            events: Vec::new(),
            garbage: 0,
            progressed: false,
        };
        for part in parts {
            if part.seq != self.next {
                // Already delivered, or a gap: the sender resends in order.
                continue;
            }
            if part.count == 0 || part.count > MAX_FRAGMENTS || part.index >= part.count {
                received.garbage += 1;
                continue;
            }
            let fresh = !matches!(&self.partial, Some((count, _)) if *count == part.count);
            if fresh {
                self.partial = Some((part.count, vec![None; usize::from(part.count)]));
            }
            let Some((_, slots)) = &mut self.partial else {
                continue;
            };
            if let Some(slot) = slots.get_mut(usize::from(part.index)) {
                if slot.is_none() {
                    received.progressed = true;
                }
                *slot = Some(part.data);
            }
            let size: usize = slots.iter().flatten().map(Vec::len).sum();
            if size > max {
                // Bigger than allowed: drop it whole, and move past it so the
                // sender's queue empties instead of resending it forever.
                received.garbage += 1;
                self.partial = None;
                self.next = self.next.saturating_add(1);
                continue;
            }
            if slots.iter().all(Option::is_some) {
                if !allow() {
                    // Over budget: keep the fragments, acknowledge nothing.
                    return received;
                }
                let data = slots
                    .iter_mut()
                    .flat_map(|slot| slot.take().unwrap_or_default())
                    .collect();
                received.events.push(Arrived {
                    data,
                    phase: part.phase,
                    at: part.at,
                });
                self.partial = None;
                self.next = self.next.saturating_add(1);
                received.progressed = true;
            }
        }
        received
    }
}

/// The latest state to send, until acknowledged.
pub(crate) struct OutState {
    seq: u32,
    data: Vec<u8>,
    acked: bool,
    sent: Vec<Option<Ms>>,
    cut: usize,
}

impl OutState {
    pub fn new() -> Self {
        OutState {
            seq: 0,
            data: Vec::new(),
            acked: true,
            sent: Vec::new(),
            cut: 0,
        }
    }

    pub fn seq(&self) -> u32 {
        self.seq
    }

    pub fn set(&mut self, data: Vec<u8>) {
        self.seq = self.seq.saturating_add(1);
        self.data = data;
        self.acked = false;
        self.sent.clear();
        self.cut = 0;
    }

    pub fn ack(&mut self, seq: u32) {
        if seq >= self.seq {
            self.acked = true;
        }
    }

    /// A new link: the latest state goes again.
    pub fn unsend(&mut self) {
        if self.seq > 0 {
            self.acked = false;
        }
        self.sent.clear();
        self.cut = 0;
    }

    pub fn due_at(&self, resend: u64) -> Option<Ms> {
        if self.acked {
            return None;
        }
        if self.sent.is_empty() {
            return Some(Ms(0));
        }
        self.sent
            .iter()
            .map(|sent| sent.map_or(Ms(0), |at| at.plus(resend)))
            .min()
    }

    pub fn take(
        &mut self,
        now: Ms,
        resend: u64,
        packet: usize,
        budget: &mut Budget,
    ) -> Vec<StatePart> {
        let mut parts = Vec::new();
        if self.acked {
            return parts;
        }
        let cut = fragment_size(packet);
        let count = fragment_count(self.data.len(), cut);
        if self.cut != cut {
            self.cut = cut;
            self.sent = vec![None; usize::from(count)];
        }
        for (index, sent) in (0..count).zip(self.sent.iter_mut()) {
            if sent.is_some_and(|at| now.since(at) < resend) {
                continue;
            }
            let data = fragment(&self.data, index, cut);
            if !budget.take(data.len()) {
                break;
            }
            *sent = Some(now);
            parts.push(StatePart {
                seq: self.seq,
                index,
                count,
                data,
            });
        }
        parts
    }
}

/// The latest state received, put back together.
pub(crate) struct InState {
    applied: u32,
    building: Option<(u32, u16, Slots)>,
}

impl InState {
    pub fn new() -> Self {
        InState {
            applied: 0,
            building: None,
        }
    }

    pub fn applied(&self) -> u32 {
        self.applied
    }

    pub fn restart(&mut self) {
        self.applied = 0;
        self.building = None;
    }

    /// The newest complete state among `parts`, if any. Older ones are
    /// ignored, even when they arrive later.
    pub fn receive(&mut self, parts: Vec<StatePart>, max: usize) -> (Option<Vec<u8>>, u32) {
        let mut done = None;
        let mut garbage = 0;
        for part in parts {
            if part.seq <= self.applied {
                continue;
            }
            if part.count == 0 || part.count > MAX_FRAGMENTS || part.index >= part.count {
                garbage += 1;
                continue;
            }
            let same = matches!(&self.building, Some((seq, count, _)) if *seq == part.seq && *count == part.count);
            if !same {
                let newer = self
                    .building
                    .as_ref()
                    .is_none_or(|(seq, _, _)| part.seq >= *seq);
                if !newer {
                    continue;
                }
                self.building = Some((part.seq, part.count, vec![None; usize::from(part.count)]));
            }
            let Some((seq, _, slots)) = &mut self.building else {
                continue;
            };
            let seq = *seq;
            if let Some(slot) = slots.get_mut(usize::from(part.index)) {
                *slot = Some(part.data);
            }
            let size: usize = slots.iter().flatten().map(Vec::len).sum();
            if size > max {
                garbage += 1;
                self.building = None;
                continue;
            }
            if slots.iter().all(Option::is_some) {
                let data = slots
                    .iter_mut()
                    .flat_map(|slot| slot.take().unwrap_or_default())
                    .collect();
                self.applied = seq;
                self.building = None;
                done = Some(data);
            }
        }
        (done, garbage)
    }
}

/// Round trip and, for the client, the host's clock, measured from the echo
/// in each packet.
pub(crate) struct Peer {
    /// The last `sent` heard from the other side, and when.
    heard: Option<(u64, Ms)>,
    rtt: Option<u64>,
    /// Recent (round trip, offset) samples; the one with the smallest round
    /// trip gives the clock offset.
    samples: VecDeque<(u64, i64)>,
}

/// Round trips above this are nonsense (a hostile echo): ignored.
const MAX_RTT: u64 = 10_000;
const SAMPLES: usize = 16;

impl Peer {
    pub fn new() -> Self {
        Peer {
            heard: None,
            rtt: None,
            samples: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Peer::new();
    }

    pub fn rtt(&self) -> Option<u64> {
        self.rtt
    }

    /// Takes a packet's timing fields. Returns the round trip it measured.
    pub fn on_packet(
        &mut self,
        now: Ms,
        sent: u64,
        echo: Option<crate::wire::Echo>,
    ) -> Option<u64> {
        self.heard = Some((sent, now));
        let echo = echo?;
        let rtt = now
            .0
            .checked_sub(echo.sent)?
            .checked_sub(u64::from(echo.held))?;
        if rtt > MAX_RTT {
            return None;
        }
        self.rtt = Some(self.rtt.map_or(rtt, |smooth| (smooth * 9 + rtt) / 10));
        // The other side's clock when this packet left, against ours.
        let theirs = i64::try_from(sent).unwrap_or(i64::MAX);
        let ours = i64::try_from(now.0).unwrap_or(i64::MAX);
        let half = i64::try_from(rtt / 2).unwrap_or(0);
        self.samples
            .push_back((rtt, theirs.saturating_add(half).saturating_sub(ours)));
        if self.samples.len() > SAMPLES {
            self.samples.pop_front();
        }
        Some(rtt)
    }

    pub fn echo(&self, now: Ms) -> Option<crate::wire::Echo> {
        let (sent, at) = self.heard?;
        Some(crate::wire::Echo {
            sent,
            held: u32::try_from(now.since(at)).unwrap_or(u32::MAX),
        })
    }

    /// The other side's clock now, if measured.
    pub fn their_time(&self, now: Ms) -> Option<u64> {
        let (_, offset) = self.samples.iter().min_by_key(|(rtt, _)| *rtt)?;
        let ours = i64::try_from(now.0).unwrap_or(i64::MAX);
        u64::try_from(ours.saturating_add(*offset)).ok()
    }

    /// How long to wait before resending, from the measured round trip.
    pub fn resend(&self, min: u64, max: u64) -> u64 {
        self.rtt
            .map_or(min.max(250), |rtt| (rtt * 2).clamp(min, max))
    }
}

/// A token bucket: `allow` says whether one more fits the budget.
pub(crate) struct Bucket {
    /// Thousandths of a token.
    milli: u64,
    refilled: Ms,
    per_s: u64,
    burst: u64,
}

impl Bucket {
    pub fn new(now: Ms, per_s: u64, burst: u64) -> Self {
        Bucket {
            milli: burst.saturating_mul(1000),
            refilled: now,
            per_s,
            burst,
        }
    }

    pub fn allow(&mut self, now: Ms) -> bool {
        let elapsed = now.since(self.refilled);
        self.refilled = now;
        self.milli = self
            .milli
            .saturating_add(elapsed.saturating_mul(self.per_s))
            .min(self.burst.saturating_mul(1000));
        if self.milli >= 1000 {
            self.milli -= 1000;
            true
        } else {
            false
        }
    }
}
