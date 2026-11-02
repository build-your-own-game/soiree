//! Reliability over a link that may lose, repeat or reorder packets, written
//! once for both sides.
//!
//! - **Events** are numbered and kept until acknowledged; every send carries
//!   the due ones from the oldest (Quake 3's reliable commands, Fiedler's
//!   reliable messages). What arrives past a hole waits for it, within
//!   [`AHEAD`] numbers, and the ack says so: the sender fills the hole without
//!   waiting for its timer, and sends nothing else again. The
//!   numbers belong to the device, not to the link: what was in flight when
//!   a link died goes again on the next one, and is not delivered twice.
//!   The host owns them: each welcome says where they stand, so a side that
//!   starts over never numbers under what the other has already seen.
//! - **The state** is the latest value only: a newer one replaces an older
//!   one, unless that one is on its way in several fragments. It finishes,
//!   then the latest takes its place: replaced at each refresh, a state of
//!   many fragments would never arrive whole. It goes again until
//!   acknowledged.
//!
//! A message bigger than one packet is cut into fragments, sized for the
//! current link when it is sent. What leaves follows what comes back
//! ([`Window`]): a link is never offered more than it has shown it carries, and
//! a link that answers nothing is asked less and less often.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec;
use alloc::vec::Vec;

use crate::types::{Caps, Ms, Timing};
use crate::wire::{Ack, EventPart, PACKET_OVERHEAD, PART_OVERHEAD, Parts, StatePart};

/// One message being put back together. It costs what arrived, never what the
/// sender announced: `count` stays a number until fragments fill it, and each
/// fragment is one step, however many there are.
struct Assembly {
    count: u16,
    parts: BTreeMap<u16, Vec<u8>>,
    size: usize,
    phase: u32,
    at: Option<u64>,
    /// Bigger than allowed: dropped, and passed over when its turn comes.
    refused: bool,
}

impl Assembly {
    fn new(count: u16) -> Assembly {
        Assembly {
            count,
            parts: BTreeMap::new(),
            size: 0,
            phase: 0,
            at: None,
            refused: false,
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
        self.refused || self.parts.len() == usize::from(self.count)
    }

    /// Lets go of all it holds. Says how many fragments that was.
    fn refuse(&mut self) -> usize {
        self.refused = true;
        self.size = 0;
        let held = self.parts.len();
        self.parts.clear();
        held
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
        Parts {
            seq,
            count: self.count,
            first,
            more,
        }
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
/// The longest message salon can carry: past [`MAX_FRAGMENTS`] on a link of
/// the smallest fragments the shells use, no receiver would take it.
pub(crate) const LARGEST_MESSAGE: usize =
    (MAX_FRAGMENTS as usize) * (Caps::DATAGRAM.max_packet - PACKET_OVERHEAD - PART_OVERHEAD);
/// Bytes of events one side keeps for the other: what [`Limits`] let wait of
/// the largest message a device may send, the default `max_pending` of the
/// default `max_up`. By default 16 events of the largest `max_down` then wait
/// for a phone that does not answer, and `Full` tells the game so.
///
/// [`Limits`]: crate::types::Limits
const MOST_WAITING: usize = 256 * 4 * 1024;
/// Numbers a receiver keeps from the next one it expects: the bits of
/// [`Ack::ahead`]. A sender does not go past them. About as many events as
/// [`Limits::events_burst`](crate::types::Limits::events_burst) lets in at once.
const AHEAD: u32 = u64::BITS;
/// Fragments kept past the next event: as many as one message of the largest
/// count. Without this bound, 64 messages announcing 1,024 empty fragments each
/// would hold 65,536 of them for nothing. The rest goes again.
const HELD_PAST_NEXT: usize = MAX_FRAGMENTS as usize;
/// Acknowledgements that pass over a fragment before it goes again without
/// waiting for its timer: three, like SCTP and TCP. Fewer, and a packet that
/// is merely late goes twice.
const REPORTS: u8 = 3;

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

/// Room left in a packet being filled, and in the window of the link.
pub(crate) struct Budget {
    pub bytes: usize,
    /// Data bytes the window still lets out.
    pub room: usize,
    /// A timer ran out on a fragment going again.
    pub expired: bool,
}

impl Budget {
    pub fn for_packet(packet: usize, room: usize) -> Budget {
        Budget {
            bytes: packet.saturating_sub(PACKET_OVERHEAD),
            room,
            expired: false,
        }
    }

    /// The last fragment that gets in may pass the window by its own size: it
    /// is one fragment, and a window that refused it would wake the sender up
    /// for nothing until the next ack.
    fn take(&mut self, data: usize) -> bool {
        self.room > 0 && self.take_again(data)
    }

    /// A fragment known lost left what is on its way, and the window waits for
    /// it: what follows cannot be delivered, so nothing else would open it.
    fn take_again(&mut self, data: usize) -> bool {
        let Some(left) = data
            .checked_add(PART_OVERHEAD)
            .and_then(|cost| self.bytes.checked_sub(cost))
        else {
            return false;
        };
        self.bytes = left;
        self.room = self.room.saturating_sub(data);
        true
    }
}

/// What a transport does with what survives it, which says how to read an ack.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
    /// A packet may overtake another: it takes a few acks to call one lost.
    Any,
    /// A stream: past what the other side heard, whatever it does not have
    /// was dropped on this side of the transport. A WebSocket loses nothing
    /// by itself, so no timer runs on it: a copy of what is on its way would
    /// only stand in front of what is fresh.
    Kept,
}

impl Order {
    pub fn of(caps: Caps) -> Order {
        if caps.in_order() {
            Order::Kept
        } else {
            Order::Any
        }
    }

    /// How many first fragments count as passed over when an ack says nothing
    /// of a message is there: on a datagram link, none, since nothing tells
    /// them from those still on their way. No timer would save a stream that
    /// took this for no news.
    fn nothing_there(self) -> usize {
        match self {
            Order::Any => 0,
            Order::Kept => usize::MAX,
        }
    }

    /// How long a fragment waits for news: on a stream, for ever.
    pub fn wait(self, peer: &Peer, window: &Window, timing: &Timing) -> u64 {
        match self {
            Order::Any => window.wait(
                peer.resend(timing.resend_min, timing.resend_max),
                timing.resend_max,
            ),
            Order::Kept => u64::MAX,
        }
    }
}

/// What an ack taught a sender.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Heard {
    /// Data bytes it had been waiting news of, and now knows arrived.
    bytes: usize,
    /// Fragments it passed over too many times.
    lost: bool,
}

impl Heard {
    pub fn with(self, other: Heard) -> Heard {
        Heard {
            bytes: self.bytes.saturating_add(other.bytes),
            lost: self.lost || other.lost,
        }
    }
}

/// How much may be on its way at once on one link, in one direction: what
/// comes back says what passes (Reno, RFC 9002 section 7). It doubles each
/// round trip, then adds a fragment each round trip past the size it last lost
/// something at, and is halved by a loss. So the queue never sits in a buffer
/// it would overflow, and a resend does not pile up behind its own original.
///
/// `str0m` says nothing of its own window, and refuses a write past 128 KiB;
/// the page's browser and the Wi-Fi have theirs, which nobody can read from
/// here. A loss is all salon learns of them. The constants below have not been
/// tried against any of them, only against the simulated pipes of the tests.
pub(crate) struct Window {
    bytes: usize,
    /// Past this, it grows by a fragment each round trip.
    ceiling: usize,
    /// Timers run out in a row with nothing heard: each doubles the wait.
    expired: u32,
    /// When a loss last halved it: everything in flight then counts as one.
    halved: Option<Ms>,
}

impl Window {
    /// Ten full packets, the first window of QUIC and TCP (RFC 9002 section 7.2).
    const START: usize = 10 * Caps::DATAGRAM.max_packet;
    /// Whatever it fell to, two packets may leave: with less, an ack per
    /// packet stops. RFC 9002 section 7.2 keeps two as well.
    const FEWEST_PACKETS: usize = 2;
    /// Three quarters of str0m's 128 KiB, which also counts headers: above it,
    /// the DataChannel refuses what comes next.
    const MOST: usize = 96 * 1024;
    /// A wait stops doubling here, far past any `Timing::resend_max`.
    const MOST_DOUBLINGS: u32 = 16;

    pub fn new() -> Window {
        Window {
            bytes: Window::START,
            ceiling: Window::MOST,
            expired: 0,
            halved: None,
        }
    }

    /// Bytes that may still leave, with `flying` on their way.
    pub fn room(&self, flying: usize, packet: usize) -> usize {
        self.bytes
            .max(packet.saturating_mul(Window::FEWEST_PACKETS))
            .saturating_sub(flying)
    }

    /// How long a fragment waits for news, `resend` being what the round trip
    /// asks for: twice as long after each timer that ran out in a row, up to
    /// `max`, as `ENet` and `quinn` do.
    pub fn wait(&self, resend: u64, max: u64) -> u64 {
        let doubled = 1_u64
            .checked_shl(self.expired)
            .map_or(u64::MAX, |times| resend.saturating_mul(times));
        doubled.min(max.max(resend))
    }

    pub fn heard(&mut self, heard: Heard, now: Ms, wait: u64) {
        if heard.bytes > 0 {
            self.expired = 0;
            let more = if self.bytes < self.ceiling {
                heard.bytes
            } else {
                heard
                    .bytes
                    .saturating_mul(Caps::DATAGRAM.max_packet)
                    .checked_div(self.bytes)
                    .unwrap_or(0)
            };
            self.bytes = self.bytes.saturating_add(more).min(Window::MOST);
        }
        if heard.lost {
            self.halve(now, wait);
        }
    }

    /// A timer ran out on something in flight.
    pub fn expired(&mut self, now: Ms, wait: u64) {
        if self.halve(now, wait) {
            self.expired = self.expired.saturating_add(1).min(Window::MOST_DOUBLINGS);
        }
    }

    fn halve(&mut self, now: Ms, wait: u64) -> bool {
        if self.halved.is_some_and(|at| now.since(at) < wait) {
            return false;
        }
        self.halved = Some(now);
        self.bytes /= 2;
        self.ceiling = self.bytes;
        true
    }
}

/// What the sender knows of one fragment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Piece {
    /// It has to leave.
    Waiting,
    /// It left then, and this many acknowledgements have passed over it.
    Sent(Ms, u8),
    /// It left, and did not arrive: it goes again, whatever the window.
    Lost,
    /// The other side said it has it. It says so of a fragment it keeps: what
    /// it drops, it drops with the link, and a new link starts [`Pieces`] over.
    Arrived,
}

/// One message on its way, in fragments of `cut` bytes.
#[derive(Default)]
struct Pieces {
    /// Bytes of the message.
    len: usize,
    cut: usize,
    each: Vec<Piece>,
}

impl Pieces {
    /// Starts over, unless the message is already cut this way.
    fn cut(&mut self, len: usize, cut: usize) {
        if self.cut != cut {
            self.len = len;
            self.cut = cut;
            self.each = vec![Piece::Waiting; usize::from(fragment_count(len, cut))];
        }
    }

    /// Bytes of a fragment: the last one may be short, and a small event is.
    fn size(&self, index: usize) -> usize {
        self.len
            .saturating_sub(index.saturating_mul(self.cut))
            .min(self.cut)
    }

    /// What is `Sent`, in bytes, among those `keep` says.
    fn weigh(&self, keep: impl Fn(Ms) -> bool) -> usize {
        self.each
            .iter()
            .enumerate()
            .filter(|(_, piece)| matches!(piece, Piece::Sent(at, _) if keep(*at)))
            .map(|(index, _)| self.size(index))
            .fold(0, usize::saturating_add)
    }

    /// Whether this fragment leaves in `budget`, which it uses up if so.
    fn fits(&self, index: usize, data: usize, budget: &mut Budget) -> bool {
        if self.each.get(index) == Some(&Piece::Lost) {
            budget.take_again(data)
        } else {
            budget.take(data)
        }
    }

    /// A new link: everything goes again, cut for it.
    fn forget(&mut self) {
        self.cut = 0;
        self.each.clear();
    }

    /// Several fragments, and some have left.
    fn under_way(&self) -> bool {
        self.each.len() > 1 && self.each.iter().any(|piece| *piece != Piece::Waiting)
    }

    /// The next fragment due now, from `from` on.
    fn due(&self, from: usize, now: Ms, resend: u64) -> Option<usize> {
        let rest = self.each.get(from..)?;
        let due = rest.iter().position(|piece| match piece {
            Piece::Waiting | Piece::Lost => true,
            Piece::Sent(at, _) => now.since(*at) >= resend,
            Piece::Arrived => false,
        })?;
        Some(from.saturating_add(due))
    }

    /// It left now. Says whether a timer sent it.
    fn sent(&mut self, index: usize, now: Ms) -> bool {
        let Some(piece) = self.each.get_mut(index) else {
            return false;
        };
        let expired = matches!(piece, Piece::Sent(..));
        *piece = Piece::Sent(now, 0);
        expired
    }

    /// When something of it goes next, if anything does: the timers only,
    /// without `room` in the window. Not cut yet: at once.
    fn due_at(&self, resend: u64, room: usize) -> Option<Ms> {
        if self.each.is_empty() {
            return (room > 0).then_some(Ms(0));
        }
        self.each
            .iter()
            .filter_map(|piece| match piece {
                Piece::Waiting => (room > 0).then_some(Ms(0)),
                Piece::Lost => Some(Ms(0)),
                Piece::Sent(at, _) => Some(at.plus(resend)),
                Piece::Arrived => None,
            })
            .min()
    }

    /// Bytes on their way, as far as the timers can tell.
    fn flying(&self, now: Ms, resend: u64) -> usize {
        self.weigh(|at| now.since(at) < resend)
    }

    /// What arrived and was on its way, in bytes.
    fn gone(&self) -> usize {
        self.weigh(|_| true)
    }

    fn all_arrived(&mut self) -> Heard {
        let bytes = self.gone();
        self.each.fill(Piece::Arrived);
        Heard { bytes, lost: false }
    }

    /// An ack that was written after `heard` left says these first `count`
    /// fragments are not there. What left after `heard` is not passed over:
    /// the ack could not have known.
    fn passed_over(&mut self, count: usize, heard: Ms, order: Order) -> Heard {
        let mut lost = false;
        for piece in self.each.iter_mut().take(count) {
            let Piece::Sent(at, reports) = piece else {
                continue;
            };
            let gone = match order {
                Order::Any if *at <= heard => {
                    *reports = reports.saturating_add(1);
                    *reports >= REPORTS
                }
                // Within the same millisecond, the order is not known.
                Order::Kept => *at < heard,
                Order::Any => false,
            };
            if gone {
                *piece = Piece::Lost;
                lost = true;
            }
        }
        Heard { bytes: 0, lost }
    }

    /// How many first fragments `parts` tells about. Of another cut, it says
    /// none of this one is there yet: only a stream can act on that, since on a
    /// datagram link they may still be on their way.
    fn holes(&self, parts: Parts, order: Order) -> usize {
        if usize::from(parts.count) == self.each.len() {
            parts.span(order)
        } else {
            order.nothing_there()
        }
    }

    /// The fragments `parts` names, if it speaks of this cut.
    fn arrived(&mut self, parts: Parts) -> Heard {
        if usize::from(parts.count) != self.each.len() {
            return Heard::default();
        }
        let first = usize::from(parts.first);
        let (len, cut) = (self.len, self.cut);
        let mut bytes = 0_usize;
        for (index, piece) in self.each.iter_mut().enumerate() {
            let past = index.checked_sub(first.saturating_add(1));
            let said = past.is_some_and(|bit| {
                u32::try_from(bit)
                    .ok()
                    .and_then(|bit| parts.more.checked_shr(bit))
                    .is_some_and(|more| more & 1 == 1)
            });
            if (index < first || said) && matches!(piece, Piece::Sent(..)) {
                let size = len.saturating_sub(index.saturating_mul(cut)).min(cut);
                bytes = bytes.saturating_add(size);
            }
            if index < first || said {
                *piece = Piece::Arrived;
            }
        }
        Heard { bytes, lost: false }
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
    /// Bytes in `queue`.
    waiting: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Full;

impl OutEvents {
    pub fn new(next: u32) -> Self {
        OutEvents {
            next,
            queue: VecDeque::new(),
            waiting: 0,
        }
    }

    pub fn push(
        &mut self,
        data: Vec<u8>,
        phase: u32,
        at: Option<u64>,
        max: usize,
    ) -> Result<u32, Full> {
        let bytes = self.waiting.saturating_add(data.len());
        let room = self.queue.is_empty() || bytes <= MOST_WAITING;
        if self.queue.len() >= max || !room {
            return Err(Full);
        }
        let seq = self.next;
        self.next = self.next.checked_add(1).ok_or(Full)?;
        self.waiting = bytes;
        self.queue.push_back(Outgoing {
            seq,
            data,
            phase,
            at,
            pieces: Pieces::default(),
        });
        Ok(seq)
    }

    /// The other side has every event below `next`.
    pub fn ack_below(&mut self, next: u32) -> Heard {
        let mut heard = Heard::default();
        while self.queue.front().is_some_and(|event| event.seq < next) {
            if let Some(event) = self.queue.pop_front() {
                heard.bytes = heard.bytes.saturating_add(event.pieces.gone());
                self.waiting = self.waiting.saturating_sub(event.data.len());
            }
        }
        heard
    }

    /// What an ack says of events, written once `heard` reached the other side.
    pub fn ack(&mut self, ack: &Ack, heard: Option<Ms>, order: Order) -> Heard {
        let mut learned = self.ack_below(ack.events);
        let parts = ack.event_parts;
        // The last number something arrived of: what lies before was passed
        // over, and nothing is known of what lies after.
        let whole = u64::BITS
            .checked_sub(ack.ahead.leading_zeros())
            .and_then(|top| top.checked_sub(1))
            .map(|top| ack.events.saturating_add(top));
        let last = whole.max(parts.map(|parts| parts.seq));
        for event in &mut self.queue {
            let bit = event.seq.saturating_sub(ack.events);
            if ack.ahead.checked_shr(bit).is_some_and(|bits| bits & 1 == 1) {
                learned = learned.with(event.pieces.all_arrived());
                continue;
            }
            let later = last.is_some_and(|last| event.seq < last);
            let mut holes = if later { usize::MAX } else { 0 };
            if let Some(parts) = parts
                && parts.seq == event.seq
            {
                learned = learned.with(event.pieces.arrived(parts));
                holes = holes.max(event.pieces.holes(parts, order));
            } else if parts.is_none() {
                // No event is half here: this one has nothing here at all.
                holes = holes.max(order.nothing_there());
            }
            if let Some(heard) = heard {
                learned = learned.with(event.pieces.passed_over(holes, heard, order));
            }
        }
        learned
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
    pub fn due_at(&self, resend: u64, room: usize) -> Option<Ms> {
        self.sendable()
            .filter_map(|event| event.pieces.due_at(resend, room))
            .min()
    }

    /// Data bytes on their way, as far as the timers can tell.
    pub fn flying(&self, now: Ms, resend: u64) -> usize {
        self.sendable()
            .map(|event| event.pieces.flying(now, resend))
            .fold(0, usize::saturating_add)
    }

    /// What the other side would keep, were it to leave now.
    fn sendable(&self) -> impl Iterator<Item = &Outgoing> {
        let last = self.oldest().saturating_add(AHEAD);
        self.queue.iter().take_while(move |event| event.seq < last)
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
        let last = self.oldest().saturating_add(AHEAD);
        for event in &mut self.queue {
            if event.seq >= last {
                break;
            }
            event.pieces.cut(event.data.len(), cut);
            let count = fragment_count(event.data.len(), cut);
            let mut from = 0;
            while let Some(due) = event.pieces.due(from, now, resend) {
                from = due.saturating_add(1);
                let index = u16::try_from(due).unwrap_or(u16::MAX);
                let data = fragment(&event.data, index, cut);
                if !event.pieces.fits(due, data.len(), budget) {
                    return parts;
                }
                budget.expired |= event.pieces.sent(due, now);
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
    /// What arrived from `next` on, up to [`AHEAD`] numbers.
    held: BTreeMap<u32, Assembly>,
    /// Fragments in `held`, for [`HELD_PAST_NEXT`].
    fragments: usize,
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
            held: BTreeMap::new(),
            fragments: 0,
        }
    }

    pub fn next(&self) -> u32 {
        self.next
    }

    /// The numbers from [`InEvents::next`] on that are here whole, bit by bit.
    pub fn ahead(&self) -> u64 {
        self.held
            .iter()
            .filter(|(_, held)| held.is_complete())
            .filter_map(|(seq, _)| seq.checked_sub(self.next))
            .filter_map(|bit| 1_u64.checked_shl(bit))
            .fold(0, |ahead, bit| ahead | bit)
    }

    /// What arrived of the first event not whole, if any.
    pub fn parts(&self) -> Option<Parts> {
        let (seq, held) = self.held.iter().find(|(_, held)| !held.is_complete())?;
        Some(held.parts(*seq))
    }

    fn forget(&mut self) {
        self.held.clear();
        self.fragments = 0;
    }

    /// Another sender takes over the numbering: what it sends under a number
    /// being put back together is not the rest of that message.
    pub fn abandon(&mut self) {
        self.forget();
    }

    /// The sender says it no longer has what is below `first`: skip ahead.
    pub fn skip_to(&mut self, first: u32) {
        if first > self.next {
            self.next = first;
            self.forget();
        }
    }

    pub fn restart(&mut self, next: u32) {
        self.next = next;
        self.forget();
    }

    /// Takes the fragments, in any order. `allow` says how many more whole
    /// events may be accepted now; past it, they wait here.
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
            let past = part.seq.checked_sub(self.next);
            if past.is_none_or(|past| past >= AHEAD) {
                // Already delivered, or too far: the sender goes in order.
                continue;
            }
            if part.count == 0 || part.count > MAX_FRAGMENTS || part.index >= part.count {
                received.garbage = received.garbage.saturating_add(1);
                continue;
            }
            if part.seq != self.next && self.fragments >= HELD_PAST_NEXT {
                continue;
            }
            let (seq, index, count) = (part.seq, part.index, part.count);
            let held = match self.held.get_mut(&seq) {
                Some(held) if held.count == count => held,
                _ => {
                    if let Some(gone) = self.held.insert(seq, Assembly::new(count)) {
                        self.fragments = self.fragments.saturating_sub(gone.parts.len());
                    }
                    let Some(held) = self.held.get_mut(&seq) else {
                        continue;
                    };
                    held
                }
            };
            if held.refused {
                continue;
            }
            (held.phase, held.at) = (part.phase, part.at);
            if held.put(index, part.data) {
                received.progressed = true;
                self.fragments = self.fragments.saturating_add(1);
            }
            if held.size > max {
                // Bigger than allowed: drop it whole, and pass over it when
                // its turn comes, so the sender's queue empties instead of
                // resending it forever.
                received.garbage = received.garbage.saturating_add(1);
                self.fragments = self.fragments.saturating_sub(held.refuse());
            }
            self.deliver(&mut received, &mut allow);
        }
        // What waited for the budget, or for a hole, which nothing resends.
        self.deliver(&mut received, &mut allow);
        received
    }

    /// Hands over what is whole from the next number on. An event over budget
    /// stays here and is not acknowledged.
    fn deliver(&mut self, received: &mut Received, allow: &mut impl FnMut() -> bool) {
        while let Some(held) = self.held.get(&self.next) {
            if !held.is_complete() || (!held.refused && !allow()) {
                return;
            }
            let Some(whole) = self.held.remove(&self.next) else {
                return;
            };
            self.fragments = self.fragments.saturating_sub(whole.parts.len());
            if !whole.refused {
                let (phase, at) = (whole.phase, whole.at);
                received.events.push(Arrived {
                    data: whole.into_message(),
                    phase,
                    at,
                });
            }
            self.next = self.next.saturating_add(1);
            received.progressed = true;
        }
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
    /// The latest one, while one of several fragments finishes.
    latest: Option<Vec<u8>>,
}

impl OutState {
    pub fn new() -> Self {
        OutState {
            seq: 0,
            data: Vec::new(),
            holds: false,
            acked: true,
            pieces: Pieces::default(),
            latest: None,
        }
    }

    /// The number of the state held, if any.
    pub fn holding(&self) -> Option<u32> {
        self.holds.then_some(self.seq)
    }

    pub fn set(&mut self, data: Vec<u8>) {
        if self.acked || !self.pieces.under_way() {
            self.replace(data);
        } else {
            self.latest = Some(data);
        }
    }

    fn replace(&mut self, data: Vec<u8>) {
        self.seq = self.seq.saturating_add(1);
        self.data = data;
        self.holds = true;
        self.acked = false;
        self.latest = None;
        self.pieces.forget();
    }

    /// The one that waited takes over.
    fn next(&mut self) {
        if let Some(latest) = self.latest.take() {
            self.replace(latest);
        }
    }

    /// What an ack says of the state, written once `heard` reached the other
    /// side.
    pub fn ack(&mut self, ack: &Ack, heard: Option<Ms>, order: Order) -> Heard {
        if self.acked {
            return Heard::default();
        }
        if ack.state >= self.seq {
            self.acked = true;
            let learned = self.pieces.all_arrived();
            self.next();
            return learned;
        }
        let mut learned = Heard::default();
        let holes = match ack.state_parts {
            Some(parts) if parts.seq == self.seq => {
                learned = self.pieces.arrived(parts);
                self.pieces.holes(parts, order)
            }
            // Nothing of it, or only of a state before it, is there.
            Some(_) | None => order.nothing_there(),
        };
        if let Some(heard) = heard {
            learned = learned.with(self.pieces.passed_over(holes, heard, order));
        }
        learned
    }

    /// A new link: the latest state goes again.
    pub fn unsend(&mut self) {
        self.next();
        if self.holds {
            self.acked = false;
        }
        self.pieces.forget();
    }

    /// Numbers above `applied`, the last number the other side took from
    /// anyone: what is held goes again, under a number it will read.
    pub fn rebase(&mut self, applied: u32) {
        self.next();
        self.seq = applied.saturating_add(u32::from(self.holds));
        self.unsend();
    }

    pub fn due_at(&self, resend: u64, room: usize) -> Option<Ms> {
        if self.acked {
            return None;
        }
        self.pieces.due_at(resend, room)
    }

    /// Data bytes on their way, as far as the timers can tell.
    pub fn flying(&self, now: Ms, resend: u64) -> usize {
        if self.acked {
            return 0;
        }
        self.pieces.flying(now, resend)
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
        // What one packet says goes whatever the window: the latest stick of
        // a pad does not wait behind a queue of events.
        if count == 1 {
            budget.room = budget.room.max(1);
        }
        let mut from = 0;
        while let Some(due) = self.pieces.due(from, now, resend) {
            from = due.saturating_add(1);
            let index = u16::try_from(due).unwrap_or(u16::MAX);
            let data = fragment(&self.data, index, cut);
            if !self.pieces.fits(due, data.len(), budget) {
                break;
            }
            budget.expired |= self.pieces.sent(due, now);
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
                // Passed over, like an event: nothing would ever end its
                // resends, and nothing newer would follow it.
                garbage = garbage.saturating_add(1);
                self.building = None;
                self.applied = seq;
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
