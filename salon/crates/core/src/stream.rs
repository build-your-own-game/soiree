//! Reliability over a link that may lose, repeat or reorder packets, written
//! once for both sides.
//!
//! - **Events** are numbered and kept until acknowledged; every send carries
//!   the due ones from the oldest, so they arrive in order with no receive
//!   buffer (Quake 3's reliable commands, Fiedler's reliable messages). The
//!   numbers belong to the device, not to the link: what was in flight when
//!   a link died goes again on the next one, and is not delivered twice.
//!   The host owns them: each welcome says where they stand, so a side that
//!   starts over never numbers under what the other has already seen.
//! - **The state** is the latest value only: a newer one replaces an older
//!   one, even half sent. It goes again until acknowledged.
//!
//! A message bigger than one packet is cut into fragments, sized for the
//! current link when it is sent.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec;
use alloc::vec::Vec;

use crate::types::Ms;
use crate::wire::{EventPart, PACKET_OVERHEAD, PART_OVERHEAD, Parts, StatePart};

/// One message being put back together. It costs what arrived, never what the
/// sender announced: `count` stays a number until fragments fill it, and each
/// fragment is one step, however many there are.
struct Assembly {
    count: u16,
    parts: BTreeMap<u16, Vec<u8>>,
    size: usize,
    phase: u32,
    at: Option<u64>,
}

impl Assembly {
    fn new(count: u16) -> Assembly {
        Assembly {
            count,
            parts: BTreeMap::new(),
            size: 0,
            phase: 0,
            at: None,
        }
    }

    /// Keeps a fragment, in the place of a copy of it. Says whether it is new.
    fn put(&mut self, index: u16, data: Vec<u8>) -> bool {
        self.size = self.size.saturating_add(data.len());
        let copy = self.parts.insert(index, data);
        if let Some(copy) = &copy {
            self.size = self.size.saturating_sub(copy.len());
        }
        copy.is_none()
    }

    fn is_complete(&self) -> bool {
        self.parts.len() == usize::from(self.count)
    }

    /// What arrived, for the sender of message `seq`.
    fn parts(&self, seq: u32) -> Parts {
        let mut first = 0;
        for index in self.parts.keys() {
            if *index != first {
                break;
            }
            first = first.saturating_add(1);
        }
        let more = self
            .parts
            .range(first.saturating_add(1)..)
            .map_while(|(index, _)| {
                let bit = index.saturating_sub(first).saturating_sub(1);
                u64::from(bit).lt(&u64::from(u64::BITS)).then_some(bit)
            })
            .fold(0_u64, |more, bit| more | 1_u64 << bit);
        Parts { seq, first, more }
    }

    fn into_message(self) -> Vec<u8> {
        let mut message = Vec::with_capacity(self.size);
        for part in self.parts.into_values() {
            message.extend_from_slice(&part);
        }
        message
    }
}

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
        let Some(left) = data
            .checked_add(PART_OVERHEAD)
            .and_then(|cost| self.bytes.checked_sub(cost))
        else {
            return false;
        };
        self.bytes = left;
        true
    }
}

/// What the sender knows of one fragment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Piece {
    /// It has to leave.
    Waiting,
    /// It left then, and nobody said what became of it.
    Sent(Ms),
    /// The other side said it has it. It says so of a fragment it keeps: what
    /// it drops, it drops with the link, and a new link starts [`Pieces`] over.
    Arrived,
}

/// One message on its way, in fragments of `cut` bytes.
#[derive(Default)]
struct Pieces {
    cut: usize,
    each: Vec<Piece>,
}

impl Pieces {
    /// Starts over, unless the message is already cut this way.
    fn cut(&mut self, len: usize, cut: usize) {
        if self.cut != cut {
            self.cut = cut;
            self.each = vec![Piece::Waiting; usize::from(fragment_count(len, cut))];
        }
    }

    /// A new link: everything goes again, cut for it.
    fn forget(&mut self) {
        self.cut = 0;
        self.each.clear();
    }

    /// The next fragment due now, from `from` on.
    fn due(&self, from: usize, now: Ms, resend: u64) -> Option<usize> {
        let rest = self.each.get(from..)?;
        let due = rest.iter().position(|piece| match piece {
            Piece::Waiting => true,
            Piece::Sent(at) => now.since(*at) >= resend,
            Piece::Arrived => false,
        })?;
        Some(from.saturating_add(due))
    }

    fn sent(&mut self, index: usize, now: Ms) {
        if let Some(piece) = self.each.get_mut(index) {
            *piece = Piece::Sent(now);
        }
    }

    /// When something of it goes again, if anything does. Not cut yet: now.
    fn due_at(&self, resend: u64) -> Option<Ms> {
        if self.each.is_empty() {
            return Some(Ms(0));
        }
        self.each
            .iter()
            .filter_map(|piece| match piece {
                Piece::Waiting => Some(Ms(0)),
                Piece::Sent(at) => Some(at.plus(resend)),
                Piece::Arrived => None,
            })
            .min()
    }

    fn arrived(&mut self, parts: Parts) {
        let first = usize::from(parts.first);
        for (index, piece) in self.each.iter_mut().enumerate() {
            let past = index.checked_sub(first.saturating_add(1));
            let said = past.is_some_and(|bit| {
                u32::try_from(bit)
                    .ok()
                    .and_then(|bit| parts.more.checked_shr(bit))
                    .is_some_and(|more| more & 1 == 1)
            });
            if index < first || said {
                *piece = Piece::Arrived;
            }
        }
    }
}

struct Outgoing {
    seq: u32,
    data: Vec<u8>,
    phase: u32,
    at: Option<u64>,
    pieces: Pieces,
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
            pieces: Pieces::default(),
        });
        Ok(seq)
    }

    /// The other side has every event below `next`, and these `parts` of one
    /// it is putting back together.
    pub fn ack(&mut self, next: u32, parts: Option<Parts>) {
        while self.queue.front().is_some_and(|event| event.seq < next) {
            self.queue.pop_front();
        }
        if let Some(parts) = parts
            && let Some(event) = self.queue.iter_mut().find(|event| event.seq == parts.seq)
        {
            event.pieces.arrived(parts);
        }
    }

    /// The oldest event still waiting (or the next number if none).
    pub fn oldest(&self) -> u32 {
        self.queue.front().map_or(self.next, |event| event.seq)
    }

    /// A new link: everything goes again, cut for it.
    pub fn unsend(&mut self) {
        for event in &mut self.queue {
            event.pieces.forget();
        }
    }

    /// Keeps what waits but numbers it from `next` (a welcome says where the
    /// host stands).
    pub fn renumber(&mut self, next: u32) {
        let mut seq = next;
        for event in &mut self.queue {
            event.seq = seq;
            seq = seq.saturating_add(1);
        }
        self.next = seq;
    }

    /// When something waiting goes again, if anything waits.
    pub fn due_at(&self, resend: u64) -> Option<Ms> {
        self.queue
            .iter()
            .filter_map(|event| event.pieces.due_at(resend))
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
            event.pieces.cut(event.data.len(), cut);
            let count = fragment_count(event.data.len(), cut);
            let mut from = 0;
            while let Some(due) = event.pieces.due(from, now, resend) {
                from = due.saturating_add(1);
                let index = u16::try_from(due).unwrap_or(u16::MAX);
                let data = fragment(&event.data, index, cut);
                if !budget.take(data.len()) {
                    return parts;
                }
                event.pieces.sent(due, now);
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
    partial: Option<Assembly>,
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

    /// What arrived of the event being put back together, if any.
    pub fn parts(&self) -> Option<Parts> {
        self.partial
            .as_ref()
            .map(|partial| partial.parts(self.next))
    }

    /// Another sender takes over the numbering: what it sends under the
    /// number being put back together is not the rest of this message.
    pub fn abandon(&mut self) {
        self.partial = None;
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
                received.garbage = received.garbage.saturating_add(1);
                continue;
            }
            let same = self.partial.as_ref().map(|partial| partial.count) == Some(part.count);
            let partial = if same {
                self.partial.as_mut()
            } else {
                Some(self.partial.insert(Assembly::new(part.count)))
            };
            let Some(partial) = partial else {
                continue;
            };
            (partial.phase, partial.at) = (part.phase, part.at);
            if partial.put(part.index, part.data) {
                received.progressed = true;
            }
            if partial.size > max {
                // Bigger than allowed: drop it whole, and move past it so the
                // sender's queue empties instead of resending it forever.
                received.garbage = received.garbage.saturating_add(1);
                self.partial = None;
                self.next = self.next.saturating_add(1);
                continue;
            }
            if self.deliver(&mut received, &mut allow).is_err() {
                return received;
            }
        }
        // What waited for the budget, which the sender no longer resends.
        let _: Result<(), Full> = self.deliver(&mut received, &mut allow);
        received
    }

    /// Hands over the event being put back together, if it is whole. Over
    /// budget it stays here and is not acknowledged: `Full`.
    fn deliver(
        &mut self,
        received: &mut Received,
        allow: &mut impl FnMut() -> bool,
    ) -> Result<(), Full> {
        if !self.partial.as_ref().is_some_and(Assembly::is_complete) {
            return Ok(());
        }
        if !allow() {
            return Err(Full);
        }
        if let Some(whole) = self.partial.take() {
            let (phase, at) = (whole.phase, whole.at);
            received.events.push(Arrived {
                data: whole.into_message(),
                phase,
                at,
            });
        }
        self.next = self.next.saturating_add(1);
        received.progressed = true;
        Ok(())
    }
}

/// The latest state to send, until acknowledged.
pub(crate) struct OutState {
    seq: u32,
    data: Vec<u8>,
    /// Something was set: a number alone says nothing, since [`OutState::rebase`]
    /// moves it.
    holds: bool,
    acked: bool,
    pieces: Pieces,
}

impl OutState {
    pub fn new() -> Self {
        OutState {
            seq: 0,
            data: Vec::new(),
            holds: false,
            acked: true,
            pieces: Pieces::default(),
        }
    }

    /// The number of the state held, if any.
    pub fn holding(&self) -> Option<u32> {
        self.holds.then_some(self.seq)
    }

    pub fn set(&mut self, data: Vec<u8>) {
        self.seq = self.seq.saturating_add(1);
        self.data = data;
        self.holds = true;
        self.acked = false;
        self.pieces.forget();
    }

    /// The other side applied `seq`, and has these `parts` of one it is
    /// putting back together.
    pub fn ack(&mut self, seq: u32, parts: Option<Parts>) {
        if seq >= self.seq {
            self.acked = true;
        }
        if let Some(parts) = parts
            && parts.seq == self.seq
        {
            self.pieces.arrived(parts);
        }
    }

    /// A new link: the latest state goes again.
    pub fn unsend(&mut self) {
        if self.holds {
            self.acked = false;
        }
        self.pieces.forget();
    }

    /// Numbers above `applied`, the last number the other side took from
    /// anyone: what is held goes again, under a number it will read.
    pub fn rebase(&mut self, applied: u32) {
        self.seq = applied.saturating_add(u32::from(self.holds));
        self.unsend();
    }

    pub fn due_at(&self, resend: u64) -> Option<Ms> {
        if self.acked {
            return None;
        }
        self.pieces.due_at(resend)
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
        self.pieces.cut(self.data.len(), cut);
        let mut from = 0;
        while let Some(due) = self.pieces.due(from, now, resend) {
            from = due.saturating_add(1);
            let index = u16::try_from(due).unwrap_or(u16::MAX);
            let data = fragment(&self.data, index, cut);
            if !budget.take(data.len()) {
                break;
            }
            self.pieces.sent(due, now);
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
    building: Option<(u32, Assembly)>,
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

    /// What arrived of the state being put back together, if any.
    pub fn parts(&self) -> Option<Parts> {
        self.building
            .as_ref()
            .map(|(seq, building)| building.parts(*seq))
    }

    pub fn restart(&mut self) {
        self.applied = 0;
        self.building = None;
    }

    /// See [`InEvents::abandon`].
    pub fn abandon(&mut self) {
        self.building = None;
    }

    /// The newest complete state among `parts`, if any. Older ones are
    /// ignored, even when they arrive later.
    pub fn receive(&mut self, parts: Vec<StatePart>, max: usize) -> (Option<Vec<u8>>, u32) {
        let mut done = None;
        let mut garbage: u32 = 0;
        for part in parts {
            if part.seq <= self.applied {
                continue;
            }
            if part.count == 0 || part.count > MAX_FRAGMENTS || part.index >= part.count {
                garbage = garbage.saturating_add(1);
                continue;
            }
            let same = self
                .building
                .as_ref()
                .map(|(seq, partial)| (*seq, partial.count))
                == Some((part.seq, part.count));
            if !same {
                let newer = self
                    .building
                    .as_ref()
                    .is_none_or(|(seq, _)| part.seq >= *seq);
                if !newer {
                    continue;
                }
                self.building = Some((part.seq, Assembly::new(part.count)));
            }
            let Some((seq, partial)) = &mut self.building else {
                continue;
            };
            let seq = *seq;
            partial.put(part.index, part.data);
            if partial.size > max {
                garbage = garbage.saturating_add(1);
                self.building = None;
                continue;
            }
            if partial.is_complete()
                && let Some((_, whole)) = self.building.take()
            {
                self.applied = seq;
                done = Some(whole.into_message());
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
        self.rtt = Some(self.rtt.map_or(rtt, |smooth| {
            smooth.saturating_mul(9).saturating_add(rtt) / 10
        }));
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
            .map_or(min.max(250), |rtt| rtt.saturating_mul(2).clamp(min, max))
    }
}

/// A token bucket: `allow` says whether one more fits the budget.
pub struct Bucket {
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
        if let Some(left) = self.milli.checked_sub(1000) {
            self.milli = left;
            true
        } else {
            false
        }
    }
}
