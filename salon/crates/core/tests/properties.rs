//! Stage 1 of `CERTIFICATION.md` for salon: random inputs and random
//! network chaos, invariants checked, failures shrunk to the smallest case.
//! `PROPTEST_CASES=5000 cargo test` searches further.

#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]
#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers outside #[test] functions: indices come from the room's own lists"
)]

#[expect(
    dead_code,
    reason = "the simulated network is shared with tests/salon.rs, which uses all of it"
)]
mod net;

use std::collections::{BTreeMap, BTreeSet};

use net::{Down, Faults, Narrow, Net, Up, phone_config};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use salon_core::{
    Caps, DeviceEvent, DeviceId, Kind, Known, LinkId, Ms, Name, Output, Presence, Status, Token,
};

/// One line per chaos case, so that a run can be counted from outside.
const CASE: &str = "chaos case";

proptest! {
    /// E1: bytes from nowhere never crash the host, never reach the game,
    /// and a stranger that only sends garbage is dropped.
    #[test]
    fn garbage_never_reaches_the_game(packets in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..300), 0..40)) {
        let mut net = Net::new(1, Faults::default());
        let link = LinkId(999);
        net.host.opened(net.now, link, Caps::DATAGRAM);
        for (n, bytes) in packets.iter().enumerate() {
            net.host.received(Ms(1_000 + n as u64), link, bytes);
            while let Some(output) = net.host.poll(Ms(1_000 + n as u64)) {
                prop_assert!(!matches!(output, Output::Device(_)), "{output:?}");
            }
        }
        prop_assert_eq!(net.host.devices().count(), 0);
    }

    /// Garbage on a welcomed link is ignored too, and the device keeps
    /// working.
    #[test]
    fn garbage_from_a_phone_is_ignored(packets in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..200), 1..20)) {
        let mut net = Net::new(2, Faults::default());
        let phone = net.add_phone(phone_config(1, Kind::Phone), Caps::DATAGRAM);
        prop_assert!(net.run_until(3_000, |net| net.phones[phone].client.status() == Status::Connected));
        let device = net.phones[phone].client.device().unwrap();
        let link = net.phones[phone].link.unwrap();
        for bytes in &packets {
            net.host.received(net.now, link, bytes);
        }
        net.phones[phone].client.send(&Up::Answer(1)).unwrap();
        prop_assert!(net.run_until(3_000, |net| net.events_from(device) == vec![Up::Answer(1)]));
    }
}

#[derive(Clone, Debug)]
enum Step {
    /// A phone sends an event.
    Send(usize),
    /// The host sends an event to a phone.
    Notify(usize),
    /// The host changes what a phone shows.
    Show(usize),
    /// A phone's transport closes.
    Cut(usize),
    /// A phone goes silent, or comes back.
    Freeze(usize),
    Thaw(usize),
    /// Time passes.
    Wait(u64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (0..3usize).prop_map(Step::Send),
        3 => (0..3usize).prop_map(Step::Notify),
        2 => (0..3usize).prop_map(Step::Show),
        1 => (0..3usize).prop_map(Step::Cut),
        1 => (0..3usize).prop_map(Step::Freeze),
        1 => (0..3usize).prop_map(Step::Thaw),
        4 => (1..3_000u64).prop_map(Step::Wait),
    ]
}

proptest! {
    // `proptest!` lets `PROPTEST_CASES` override this field (`contextualize_config`).
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Whatever the network does, every event arrives once and in order,
    /// every phone ends up connected as itself once the network is back,
    /// and a device never has two links.
    #[test]
    fn chaos_never_loses_or_doubles_an_event(
        seed in any::<u64>(),
        loss in 0..40u64,
        jitter in 0..120u64,
        steps in prop::collection::vec(step(), 1..60),
    ) {
        println!("{CASE}");
        let mut net = Net::new(seed, Faults { loss, duplicate: 10, jitter });
        for token in 0..3u8 {
            net.add_phone(phone_config(token + 1, Kind::Phone), Caps::DATAGRAM);
        }
        net.run(500);
        let mut sent_up = [0u32; 3];
        let mut sent_down = [0u32; 3];
        let mut shown = [0u32; 3];
        for step in steps {
            match step {
                Step::Send(phone) => {
                    if net.phones[phone].client.send(&Up::Answer(sent_up[phone])).is_ok() {
                        sent_up[phone] += 1;
                    }
                }
                Step::Notify(phone) => {
                    if let Some(device) = net.phones[phone].client.device()
                        && net.host.send(device, &Down::Note(sent_down[phone])).is_ok()
                    {
                        sent_down[phone] += 1;
                    }
                }
                Step::Show(phone) => {
                    if let Some(device) = net.phones[phone].client.device() {
                        shown[phone] += 1;
                        net.host.show(device, &Down::View(shown[phone], vec![0; 3_000])).unwrap();
                    }
                }
                Step::Cut(phone) => net.cut(phone),
                Step::Freeze(phone) => net.phones[phone].frozen = true,
                Step::Thaw(phone) => net.phones[phone].frozen = false,
                Step::Wait(ms) => net.run(ms),
            }
        }
        // The network heals: everybody must come back and catch up.
        net.faults = Faults::default();
        for phone in &mut net.phones {
            phone.frozen = false;
        }
        let healed = net.run_until(30_000, |net| {
            net.phones.iter().all(|phone| phone.client.status() == Status::Connected)
        });
        prop_assert!(healed, "{:?}", net.phones.iter().map(|p| p.client.status()).collect::<Vec<_>>());
        net.run(3_000);

        for (index, phone) in net.phones.iter().enumerate() {
            let device = phone.client.device().unwrap();
            let want: Vec<Up> = (0..sent_up[index]).map(Up::Answer).collect();
            prop_assert_eq!(net.events_from(device), want, "phone {}", index);
            let want: Vec<Down> = (0..sent_down[index]).map(Down::Note).collect();
            prop_assert_eq!(&phone.events, &want, "phone {}", index);
            if shown[index] > 0 {
                prop_assert_eq!(
                    phone.states.last(),
                    Some(&Down::View(shown[index], vec![0; 3_000])),
                    "phone {}", index
                );
            }
            prop_assert_eq!(phone.resets, 0);
        }
        // Each device arrived once; a Back always follows a Left.
        let mut away = std::collections::BTreeMap::new();
        for (_, event) in &net.happened {
            match event {
                DeviceEvent::Arrived { device, .. } => {
                    prop_assert!(away.insert(*device, false).is_none(), "{device} arrived twice");
                }
                DeviceEvent::Left { device, .. } => {
                    prop_assert_eq!(away.insert(*device, true), Some(false), "{} left twice", device);
                }
                DeviceEvent::Back { device } => {
                    prop_assert_eq!(away.insert(*device, false), Some(true), "{} back without leaving", device);
                }
                DeviceEvent::Event { .. } | DeviceEvent::State { .. } => {}
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Size {
    OnePacket,
    SeveralPackets,
}

/// Bytes of the biggest message the chaos sends down: under `Limits::max_down`
/// once encoded, like the biggest a game may send.
const PARCEL: usize = 60_000;

/// A message that says its number. All of a big one says the same, so that
/// pieces of two messages show.
fn message(id: u32, size: Size) -> Up {
    match size {
        Size::OnePacket => Up::Answer(id),
        Size::SeveralPackets => Up::Blob(id.to_le_bytes().repeat(600)),
    }
}

/// The number a message says.
fn number(message: &Up) -> u32 {
    match message {
        Up::Answer(id) => *id,
        Up::Blob(bytes) => {
            let id = u32::from_le_bytes(bytes[..4].try_into().unwrap());
            assert_eq!(*bytes, id.to_le_bytes().repeat(600), "pieces of two");
            id
        }
        Up::Pad(..) => panic!("the chaos sends no pad"),
    }
}

/// One page of a seat, alive.
#[derive(Debug)]
struct Page {
    phone: usize,
    /// The events it sent, in order, and the states it set.
    sent: Vec<u32>,
    states: Vec<u32>,
    /// The player left, and did not ask to come back.
    quit: bool,
    /// How many times a player asked a stopped page to play.
    asked: usize,
}

/// Everything that ever showed one token.
#[derive(Debug, Default)]
struct Seat {
    pages: Vec<Page>,
    /// A page died in silence: its twins stay replaced until a player asks.
    crashed: bool,
    /// The host lost this token: it may come back under another number.
    renumbered: bool,
}

impl Seat {
    fn page(&mut self, which: usize) -> &mut Page {
        let index = which % self.pages.len();
        &mut self.pages[index]
    }
}

/// What can happen to a room, on top of the network. `usize` pairs are a
/// seat, then which of its pages.
#[derive(Clone, Copy, Debug)]
enum Trouble {
    /// An event, or a state, that may need several packets.
    Send(usize, usize, Size),
    SetState(usize, usize, Size),
    Notify(usize),
    /// An event as big as the host may send: dozens of packets.
    Parcel(usize),
    /// The host sends more events than a device may have waiting.
    Flood(usize),
    /// What the device shows, in one packet or many.
    Show(usize, Size),
    Cut(usize, usize),
    Freeze(usize, usize),
    Thaw(usize, usize),
    /// The browser freezes the page: nothing of it runs.
    Doze(usize, usize),
    /// It thaws, and is told so, or finds out by its own timers.
    Rouse(usize, usize, bool),
    /// The page comes back to the foreground.
    Wake(usize, usize),
    /// The browser says the network is back.
    NetworkBack(usize, usize),
    /// The transport dies, and the hello of the next attempt stays in its
    /// socket for this long, while the page gives up and tries again.
    Stall(usize, usize, u64),
    /// The page reloads: gone with what it had not sent.
    Reload(usize, usize),
    /// The page dies without a word, and another one starts.
    Crash(usize, usize),
    /// Another tab opens on the same token.
    Twin(usize),
    /// A stopped tab plays again.
    PlayHere(usize, usize),
    /// The next transport of this page: a WebSocket, or WebRTC.
    Transport(usize, usize, bool),
    /// The player leaves.
    Quit(usize, usize),
    /// The game removes the device.
    Forget(usize),
    /// More events at once than the host accepts in a burst.
    Burst(usize, usize),
    /// The host dies, having lost what `Lost` says of its identities, and
    /// stays away this long.
    RestartHost(Lost, u64),
    /// The host says it stops, keeps running this long, and restarts.
    ShutdownHost(u64),
    /// The host's process stands still this long, then finds all that came.
    HostNap(u64),
    /// One packet comes late.
    Late(u64),
    Wait(u64),
}

/// What the file of identities had lost when the host came back.
#[derive(Clone, Copy, Debug)]
enum Lost {
    Nothing,
    /// A crash before this seat was saved.
    Seat(usize),
    Everything,
}

fn lost() -> impl Strategy<Value = Lost> {
    prop_oneof![
        6 => Just(Lost::Nothing),
        1 => (0..3usize).prop_map(Lost::Seat),
        1 => Just(Lost::Everything),
    ]
}

fn trouble() -> impl Strategy<Value = Trouble> {
    let seat = || 0..3usize;
    let page = || 0..4usize;
    let size = || prop_oneof![Just(Size::OnePacket), Just(Size::SeveralPackets)];
    prop_oneof![
        5 => (seat(), page(), size()).prop_map(|(s, p, k)| Trouble::Send(s, p, k)),
        3 => (seat(), page(), size()).prop_map(|(s, p, k)| Trouble::SetState(s, p, k)),
        2 => seat().prop_map(Trouble::Notify),
        1 => seat().prop_map(Trouble::Parcel),
        1 => seat().prop_map(Trouble::Flood),
        1 => (seat(), size()).prop_map(|(s, k)| Trouble::Show(s, k)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Cut(s, p)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Freeze(s, p)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Thaw(s, p)),
        2 => (seat(), page()).prop_map(|(s, p)| Trouble::Doze(s, p)),
        2 => (seat(), page(), any::<bool>()).prop_map(|(s, p, told)| Trouble::Rouse(s, p, told)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Wake(s, p)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::NetworkBack(s, p)),
        2 => (seat(), page(), 2_500..5_000u64).prop_map(|(s, p, ms)| Trouble::Stall(s, p, ms)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Reload(s, p)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Crash(s, p)),
        2 => seat().prop_map(Trouble::Twin),
        2 => (seat(), page()).prop_map(|(s, p)| Trouble::PlayHere(s, p)),
        1 => (seat(), page(), any::<bool>()).prop_map(|(s, p, ws)| Trouble::Transport(s, p, ws)),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Quit(s, p)),
        1 => seat().prop_map(Trouble::Forget),
        1 => (seat(), page()).prop_map(|(s, p)| Trouble::Burst(s, p)),
        1 => (lost(), prop_oneof![Just(0), 1..4_000u64])
            .prop_map(|(lost, ms)| Trouble::RestartHost(lost, ms)),
        1 => (0..1_500u64).prop_map(Trouble::ShutdownHost),
        1 => (1..9_000u64).prop_map(Trouble::HostNap),
        2 => (1..2_500u64).prop_map(Trouble::Late),
        5 => (1..2_000u64).prop_map(Trouble::Wait),
    ]
}

/// A room in the chaos, and what the game and the pages must end up with.
struct Room {
    net: Net,
    seats: [Seat; 3],
    next_id: u32,
    epoch: u32,
    /// Where each run of the host starts in `net.happened`.
    runs: Vec<usize>,
    /// What the host sent since its last start, per seat.
    notes: [Vec<u32>; 3],
    shown: [Option<u32>; 3],
}

impl Room {
    fn new(net: Net) -> Room {
        let mut room = Room {
            net,
            seats: [Seat::default(), Seat::default(), Seat::default()],
            next_id: 0,
            epoch: 1,
            runs: vec![0],
            notes: [Vec::new(), Vec::new(), Vec::new()],
            shown: [None; 3],
        };
        for seat in 0..3 {
            room.open_page(seat);
        }
        room
    }

    fn open_page(&mut self, seat: usize) {
        let phone = self
            .net
            .add_phone(phone_config(seat_byte(seat), Kind::Phone), Caps::DATAGRAM);
        self.seats[seat].pages.push(Page {
            phone,
            sent: Vec::new(),
            states: Vec::new(),
            quit: false,
            asked: 0,
        });
    }

    fn fresh_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    /// Who the host says this seat is now. A page's memory may be of an
    /// earlier run, where its number can belong to someone else.
    fn device(&self, seat: usize) -> Option<DeviceId> {
        let token = Token([seat_byte(seat); 16]);
        let known = self.net.host.known();
        known
            .iter()
            .find_map(|known| (known.token == token).then_some(known.device))
    }

    /// The phone of this page, if the page runs: a frozen one does nothing.
    fn awake(&mut self, seat: usize, which: usize) -> Option<usize> {
        let phone = self.seats[seat].page(which).phone;
        (!self.net.phones[phone].asleep).then_some(phone)
    }

    /// Sends a new event from this page, and says whether it took it.
    fn send(&mut self, seat: usize, which: usize, size: Size) -> bool {
        let id = self.fresh_id();
        let Some(phone) = self.awake(seat, which) else {
            return false;
        };
        let taken = self.net.phones[phone]
            .client
            .send(&message(id, size))
            .is_ok();
        if taken {
            self.seats[seat].page(which).sent.push(id);
        }
        taken
    }

    /// What the host will know of itself when it comes back.
    fn lose(&mut self, lost: Lost) -> Vec<Known> {
        let mut known = self.net.host.known();
        match lost {
            Lost::Nothing => {}
            Lost::Seat(seat) => {
                let token = Token([seat_byte(seat); 16]);
                known.retain(|known| known.token != token);
                self.seats[seat].renumbered = true;
            }
            Lost::Everything => {
                known.clear();
                for seat in &mut self.seats {
                    seat.renumbered = true;
                }
            }
        }
        known
    }

    /// A new run of the host.
    fn start(&mut self, known: Vec<Known>) {
        self.epoch += 2;
        self.net.restart_host(self.epoch, known);
        self.runs.push(self.net.happened.len());
        self.notes = [Vec::new(), Vec::new(), Vec::new()];
        self.shown = [None; 3];
    }

    fn apply(&mut self, trouble: Trouble) {
        self.act(trouble);
        self.net.stir();
    }

    #[expect(clippy::too_many_lines, reason = "one short arm for each trouble")]
    fn act(&mut self, trouble: Trouble) {
        match trouble {
            Trouble::Send(seat, which, size) => {
                self.send(seat, which, size);
            }
            Trouble::Burst(seat, which) => {
                for _ in 0..80 {
                    if !self.send(seat, which, Size::OnePacket) {
                        break;
                    }
                }
            }
            Trouble::SetState(seat, which, size) => {
                let id = self.fresh_id();
                let Some(phone) = self.awake(seat, which) else {
                    return;
                };
                self.seats[seat].page(which).states.push(id);
                self.net.phones[phone]
                    .client
                    .set_state(&message(id, size))
                    .unwrap();
            }
            Trouble::Notify(seat) => {
                let id = self.fresh_id();
                if let Some(device) = self.device(seat)
                    && self.net.host.send(device, &Down::Note(id)).is_ok()
                {
                    self.notes[seat].push(id);
                }
            }
            Trouble::Parcel(seat) => {
                let id = self.fresh_id();
                if let Some(device) = self.device(seat)
                    && self
                        .net
                        .host
                        .send(device, &Down::View(id, vec![0; PARCEL]))
                        .is_ok()
                {
                    self.notes[seat].push(id);
                }
            }
            Trouble::Flood(seat) => {
                for _ in 0..300 {
                    let id = self.fresh_id();
                    let Some(device) = self.device(seat) else {
                        return;
                    };
                    if self.net.host.send(device, &Down::Note(id)).is_err() {
                        return;
                    }
                    self.notes[seat].push(id);
                }
            }
            Trouble::Show(seat, size) => {
                let id = self.fresh_id();
                let bytes = match size {
                    Size::OnePacket => 500,
                    Size::SeveralPackets => PARCEL,
                };
                if let Some(device) = self.device(seat)
                    && self
                        .net
                        .host
                        .show(device, &Down::View(id, vec![0; bytes]))
                        .is_ok()
                {
                    self.shown[seat] = Some(id);
                }
            }
            Trouble::Cut(seat, which) => {
                if let Some(phone) = self.awake(seat, which) {
                    self.net.cut(phone);
                }
            }
            Trouble::Freeze(seat, which) => {
                let phone = self.seats[seat].page(which).phone;
                self.net.phones[phone].frozen = true;
            }
            Trouble::Thaw(seat, which) => {
                let phone = self.seats[seat].page(which).phone;
                self.net.phones[phone].frozen = false;
            }
            Trouble::Doze(seat, which) => {
                let phone = self.seats[seat].page(which).phone;
                self.net.phones[phone].asleep = true;
            }
            Trouble::Rouse(seat, which, told) => {
                let phone = self.seats[seat].page(which).phone;
                self.net.phones[phone].asleep = false;
                if told {
                    let now = self.net.now;
                    self.net.phones[phone].client.wake(now);
                }
            }
            Trouble::Wake(seat, which) => {
                if let Some(phone) = self.awake(seat, which) {
                    let now = self.net.now;
                    self.net.phones[phone].client.wake(now);
                }
            }
            Trouble::NetworkBack(seat, which) => {
                if let Some(phone) = self.awake(seat, which) {
                    let now = self.net.now;
                    self.net.phones[phone].client.network_back(now);
                }
            }
            Trouble::Stall(seat, which, ms) => {
                let Some(phone) = self.awake(seat, which) else {
                    return;
                };
                self.net.cut(phone);
                self.net.phones[phone].held = ms;
                self.net.run(ms - 400);
                let now = self.net.now;
                self.net.phones[phone].client.network_back(now);
            }
            Trouble::Reload(seat, which) => {
                let index = which % self.seats[seat].pages.len();
                let phone = self.seats[seat].pages.remove(index).phone;
                self.net.leave(phone);
                self.open_page(seat);
            }
            Trouble::Crash(seat, which) => {
                let index = which % self.seats[seat].pages.len();
                let phone = self.seats[seat].pages.remove(index).phone;
                self.net.phones[phone].frozen = true;
                self.seats[seat].crashed = true;
                self.open_page(seat);
            }
            Trouble::Twin(seat) => self.open_page(seat),
            Trouble::PlayHere(seat, which) => {
                if self.awake(seat, which).is_some() {
                    self.ask(seat, which);
                }
            }
            Trouble::Transport(seat, which, websocket) => {
                let phone = self.seats[seat].page(which).phone;
                self.net.phones[phone].caps = if websocket {
                    Caps::STREAM
                } else {
                    Caps::DATAGRAM
                };
            }
            Trouble::Quit(seat, which) => {
                if let Some(phone) = self.awake(seat, which) {
                    self.net.phones[phone].client.quit();
                    self.seats[seat].page(which).quit = true;
                }
            }
            Trouble::Forget(seat) => {
                if let Some(device) = self.device(seat) {
                    self.net.host.forget(device);
                    self.seats[seat].renumbered = true;
                    self.notes[seat].clear();
                    self.shown[seat] = None;
                }
            }
            Trouble::RestartHost(lost, ms) => {
                let known = self.lose(lost);
                // Nothing of the old process answers any more.
                self.net.restart_host(self.epoch + 1, known.clone());
                self.net.process.gone = true;
                self.net.run(ms);
                self.net.process.gone = false;
                self.start(known);
            }
            Trouble::ShutdownHost(ms) => {
                self.net.host.shutdown();
                self.net.stir();
                self.net.run(ms);
                let known = self.lose(Lost::Nothing);
                self.start(known);
            }
            Trouble::HostNap(ms) => {
                self.net.process.asleep = true;
                self.net.run(ms);
                self.net.process.asleep = false;
            }
            Trouble::Late(ms) => self.net.hold_next(ms),
            Trouble::Wait(ms) => self.net.run(ms),
        }
    }

    /// A player asks this page to play, if it stands still.
    fn ask(&mut self, seat: usize, which: usize) {
        let page = self.seats[seat].page(which);
        page.quit = false;
        let phone = page.phone;
        if stopped(self.net.phones[phone].client.status()) {
            self.seats[seat].page(which).asked += 1;
        }
        self.net.phones[phone].client.resume();
    }

    /// Each run, split: what the game heard from that start of the host.
    fn runs(&self) -> impl Iterator<Item = &[(Ms, DeviceEvent<Up>)]> {
        let ends = self
            .runs
            .iter()
            .skip(1)
            .copied()
            .chain([self.net.happened.len()]);
        self.runs
            .iter()
            .zip(ends)
            .map(|(start, end)| &self.net.happened[*start..end])
    }
}

/// The byte of a seat's token.
fn seat_byte(seat: usize) -> u8 {
    u8::try_from(seat).unwrap() + 1
}

/// A page in this state does nothing until someone asks.
fn stopped(status: Status) -> bool {
    matches!(status, Status::Replaced | Status::Refused(_) | Status::Quit)
}

/// The network heals, and each page plays in turn: what waits in it must go.
/// Asking is what a player does on a page told it was replaced, so it comes
/// after [`settled`], which holds no page there without a reason.
fn heal(room: &mut Room) -> Result<(), TestCaseError> {
    // The network heals, and nobody is frozen any more.
    room.net.faults = Faults::default();
    for seat in &room.seats {
        for page in &seat.pages {
            room.net.phones[page.phone].frozen = false;
            room.net.phones[page.phone].asleep = false;
        }
    }
    room.net.stir();
    // Long enough for a page that thinks it plays to find out.
    room.net.run(8_000);
    // A player who left stays gone, and the room said why it turned one away:
    // these two come back when someone asks.
    for seat in 0..3 {
        for index in 0..room.seats[seat].pages.len() {
            let page = &room.seats[seat].pages[index];
            let status = room.net.phones[page.phone].client.status();
            prop_assert!(
                !page.quit || status == Status::Quit,
                "seat {seat}, page {}: left, and is {status:?}",
                page.phone
            );
            if matches!(status, Status::Refused(_) | Status::Quit) {
                room.ask(seat, index);
            }
        }
    }
    room.net.stir();
    prop_assert!(
        room.net.run_until(30_000, |net| settled(&room.seats, net)),
        "{:?}",
        room.seats
            .iter()
            .map(|seat| seat
                .pages
                .iter()
                .map(|page| room.net.phones[page.phone].client.status())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    // Each page plays in turn: what waits in it must go.
    for seat in 0..3 {
        for index in 0..room.seats[seat].pages.len() {
            let phone = room.seats[seat].pages[index].phone;
            room.ask(seat, index);
            room.net.stir();
            prop_assert!(
                room.net
                    .run_until(10_000, |net| net.phones[phone].client.status()
                        == Status::Connected),
                "page {} of seat {seat}: {:?}",
                index,
                room.net.phones[phone].statuses
            );
            room.net.run(1_500);
            // Through a narrow link, the page before this one hears it was
            // replaced behind all that was queued for it, and until then a
            // player has nothing to ask of it.
            if room.net.narrow.is_some() {
                room.net
                    .run_until(LONGEST_DRAIN, |net| settled(&room.seats, net));
            }
            // The host takes 30 events a second: what a burst left waits here.
            let sent: BTreeSet<u32> = room.seats[seat].pages[index].sent.iter().copied().collect();
            for _ in 0..40 {
                if heard_all(&room.net, &sent) {
                    break;
                }
                room.net.run(500);
            }
        }
    }
    room.net.run(3_000);
    // A narrow link takes its time, and only its time: past this, a message
    // that has not arrived is stuck. The slowest pipe of `narrow()` passes
    // 2 MB in that time, more than the longest list of troubles asks of it.
    if room.net.narrow.is_some() {
        let mut waited = 0;
        while waited < LONGEST_DRAIN && !drained(room) {
            room.net.run(500);
            waited += 500;
        }
    }
    Ok(())
}

/// The longest a room gets to empty its queues through a narrow pipe.
const LONGEST_DRAIN: u64 = 180_000;

/// Each seat has every event the host sent it, and shows the latest state.
fn drained(room: &Room) -> bool {
    room.seats.iter().enumerate().all(|(index, seat)| {
        let last = seat.pages.last().map(|page| &room.net.phones[page.phone]);
        let device = last.and_then(|phone| phone.client.device());
        let got: BTreeSet<u32> = room
            .net
            .phones
            .iter()
            .filter(|phone| phone.client.device() == device)
            .flat_map(|phone| &phone.events)
            .map(noted)
            .collect();
        let shown = room.shown[index]
            .is_none_or(|id| last.and_then(|phone| phone.states.last()).map(noted) == Some(id));
        shown && room.notes[index].iter().all(|id| got.contains(id))
    })
}

/// Every event of `sent` reached a run of the host.
fn heard_all(net: &Net, sent: &BTreeSet<u32>) -> bool {
    let heard = net.happened.iter().filter_map(|(_, event)| {
        if let DeviceEvent::Event { event, .. } = event {
            Some(number(event))
        } else {
            None
        }
    });
    sent.is_subset(&heard.collect())
}

/// Each run of the host, on its own: what it heard. Once each, in order from
/// each page, and every device comes and goes as it should.
fn check_runs(room: &Room) -> Result<BTreeSet<u32>, TestCaseError> {
    let mut heard = BTreeSet::new();
    let owner: BTreeMap<u32, usize> = room
        .seats
        .iter()
        .flat_map(|seat| &seat.pages)
        .flat_map(|page| {
            page.sent
                .iter()
                .chain(&page.states)
                .map(|id| (*id, page.phone))
        })
        .collect();
    for run in room.runs() {
        // Once per run, and in order from each page.
        let mut once = BTreeSet::new();
        let mut last: BTreeMap<(DeviceId, usize), u32> = BTreeMap::new();
        let mut newest: BTreeMap<(DeviceId, usize), u32> = BTreeMap::new();
        let mut away = BTreeMap::new();
        for (_, event) in run {
            match event {
                DeviceEvent::Event { device, event, .. } => {
                    prop_assert_eq!(away.get(device), Some(&false), "{} not here", device);
                    let id = number(event);
                    // A device removed is a stranger when it comes back: what
                    // it had in flight goes again, under its new number.
                    prop_assert!(once.insert((*device, id)), "event {} twice in one run", id);
                    heard.insert(id);
                    if let Some(page) = owner.get(&id) {
                        let before = last.insert((*device, *page), id).unwrap_or(0);
                        prop_assert!(before < id, "page {}: {} after {}", page, id, before);
                    }
                }
                DeviceEvent::Arrived { device, .. } => {
                    prop_assert!(
                        away.insert(*device, false).is_none(),
                        "{device} arrived twice"
                    );
                }
                DeviceEvent::Left { device, .. } => {
                    prop_assert_eq!(
                        away.insert(*device, true),
                        Some(false),
                        "{} left twice",
                        device
                    );
                }
                DeviceEvent::Back { device } => {
                    prop_assert_eq!(
                        away.insert(*device, false),
                        Some(true),
                        "{} back without leaving",
                        device
                    );
                }
                DeviceEvent::State { device, state } => {
                    prop_assert_eq!(away.get(device), Some(&false), "{} not here", device);
                    let id = number(state);
                    if let Some(page) = owner.get(&id) {
                        // The one it holds goes again when it plays again.
                        let before = newest.insert((*device, *page), id).unwrap_or(0);
                        prop_assert!(before <= id, "page {}: state {} after {}", page, id, before);
                    }
                }
            }
        }
    }
    Ok(heard)
}

/// Salon never called this seat by two numbers, nor said a page's past was
/// void, unless the host really lost it. A page stopped only when told, and
/// came back only when asked.
fn check_memory(room: &Room) -> Result<(), TestCaseError> {
    let restarted = room.runs.len() > 1;
    for (index, seat) in room.seats.iter().enumerate() {
        let name = Name::new(&format!("Phone {}", seat_byte(index)));
        let mut numbers = room.net.happened.iter().filter_map(|(_, event)| {
            if let DeviceEvent::Arrived {
                device, name: says, ..
            } = event
                && *says == name
            {
                Some(*device)
            } else {
                None
            }
        });
        if let (Some(first), false) = (numbers.next(), seat.renumbered) {
            prop_assert!(
                numbers.all(|number| number == first),
                "seat {} renumbered",
                index
            );
        }
        for page in &seat.pages {
            let phone = &room.net.phones[page.phone];
            if !restarted && !seat.renumbered {
                prop_assert_eq!(phone.resets, 0, "seat {}, page {}", index, page.phone);
            }
            let starts = phone
                .statuses
                .windows(2)
                .filter(|pair| stopped(pair[0]) && !stopped(pair[1]))
                .count();
            prop_assert!(
                starts <= page.asked,
                "seat {}, page {}: {} starts for {} asked: {:?}",
                index,
                page.phone,
                starts,
                page.asked,
                phone.statuses
            );
        }
    }
    Ok(())
}

/// The number of an event the host sent, of any size.
fn noted(event: &Down) -> u32 {
    match event {
        Down::Note(id) | Down::View(id, _) => *id,
    }
}

/// What must be true at the end, for each seat.
fn check_seats(room: &Room, heard: &BTreeSet<u32>) -> Result<(), TestCaseError> {
    for (index, seat) in room.seats.iter().enumerate() {
        for page in &seat.pages {
            for id in &page.sent {
                prop_assert!(
                    heard.contains(id),
                    "seat {index}, page {}: event {id} lost",
                    page.phone
                );
            }
        }
        // The page that played last is what the game and the screen know.
        let last = seat.pages.last().unwrap();
        let phone = &room.net.phones[last.phone];
        let device = phone.client.device().unwrap();
        prop_assert_eq!(room.device(index), Some(device), "seat {}", index);
        for page in &seat.pages {
            let other = room.net.phones[page.phone].client.device();
            prop_assert_eq!(other, Some(device), "seat {}: two numbers", index);
        }
        prop_assert_eq!(phone.client.status(), Status::Connected);
        prop_assert_eq!(
            room.net.host.device(device).map(|info| info.presence),
            Some(Presence::Present)
        );
        if let Some(&id) = last.states.last() {
            let state = room.net.happened.iter().rev().find_map(|(_, event)| {
                if let DeviceEvent::State {
                    device: from,
                    state,
                } = event
                    && *from == device
                {
                    Some(state)
                } else {
                    None
                }
            });
            prop_assert_eq!(state.map(number), Some(id), "seat {}", index);
        }
        if let Some(id) = room.shown[index] {
            let view = phone.states.last().map(|state| match state {
                Down::View(id, _) => *id,
                Down::Note(id) => panic!("note {id} as a state"),
            });
            prop_assert_eq!(view, Some(id), "seat {}", index);
        }
        // Whatever page got them, the notes of this run all arrived.
        let got: BTreeSet<u32> = room
            .net
            .phones
            .iter()
            .filter(|other| other.client.device() == Some(device))
            .flat_map(|other| &other.events)
            .map(noted)
            .collect();
        for id in &room.notes[index] {
            prop_assert!(got.contains(id), "seat {index}: note {id} lost");
        }
    }
    prop_assert_eq!(room.net.host.devices().count(), 3, "one device per token");
    for phone in &room.net.phones {
        let notes: Vec<u32> = phone.events.iter().map(noted).collect();
        prop_assert!(notes.windows(2).all(|pair| pair[0] < pair[1]), "{notes:?}");
        let views: Vec<u32> = phone
            .states
            .iter()
            .filter_map(|state| match state {
                Down::View(id, _) => Some(*id),
                Down::Note(_) => None,
            })
            .collect();
        prop_assert!(views.windows(2).all(|pair| pair[0] < pair[1]), "{views:?}");
    }
    Ok(())
}

/// When the host and the pages look at what they have to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Polling {
    EveryStep,
    /// As the shells do.
    AtDeadlines,
}

/// Everything a page can go through at once: the promise of `Host` and
/// `Client` when nothing is as it should be.
fn play(
    seed: u64,
    loss: u64,
    jitter: u64,
    polling: Polling,
    narrow: Option<Narrow>,
    troubles: Vec<Trouble>,
) -> Result<(), TestCaseError> {
    let mut net = Net::new(
        seed,
        Faults {
            loss,
            duplicate: 10,
            jitter,
        },
    );
    net.lazy = polling == Polling::AtDeadlines;
    net.narrow = narrow;
    let mut room = Room::new(net);
    room.net.run(500);
    for trouble in troubles {
        room.apply(trouble);
    }
    heal(&mut room)?;
    let heard = check_runs(&room)?;
    check_seats(&room, &heard)?;
    check_memory(&room)
}

/// Exactly one page of a seat plays, and the others wait to be asked. With a
/// dead page in the way, none may play until someone asks. A page alone is
/// never replaced: nothing but its own past shows its token.
fn settled(seats: &[Seat; 3], net: &Net) -> bool {
    seats.iter().all(|seat| {
        let statuses = seat
            .pages
            .iter()
            .map(|page| net.phones[page.phone].client.status());
        let waiting = statuses
            .clone()
            .all(|status| matches!(status, Status::Connected | Status::Replaced));
        let playing = statuses
            .filter(|status| *status == Status::Connected)
            .count();
        let alone = seat.pages.len() == 1;
        waiting && (playing == 1 || (playing == 0 && seat.crashed && !alone))
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// A reload, a twin, a crash, a frozen page, a player who leaves, a device
    /// removed, a change of transport, a restart of the host, a packet held
    /// back: mixed with the network's own faults, every one at any moment,
    /// and half the time polled as the shells poll.
    #[test]
    fn chaos_keeps_every_page_in_its_place(
        seed in any::<u64>(),
        loss in 0..30u64,
        jitter in 0..100u64,
        polling in prop_oneof![Just(Polling::EveryStep), Just(Polling::AtDeadlines)],
        troubles in prop::collection::vec(trouble(), 1..50),
    ) {
        play(seed, loss, jitter, polling, None, troubles)?;
    }

    /// The same, on links that carry so much and no more: a message bigger
    /// than what passes between two resends goes through like any other.
    #[test]
    fn chaos_keeps_every_page_in_its_place_through_a_narrow_pipe(
        seed in any::<u64>(),
        loss in 0..30u64,
        jitter in 0..100u64,
        polling in prop_oneof![Just(Polling::EveryStep), Just(Polling::AtDeadlines)],
        narrow in narrow(),
        troubles in prop::collection::vec(trouble(), 1..50),
    ) {
        play(seed, loss, jitter, polling, Some(narrow), troubles)?;
    }
}

/// Links narrower than the messages of the chaos, each with the buffer in
/// front of it. The rates are chosen to be below one [`PARCEL`] a second, not
/// measured on a real network.
fn narrow() -> impl Strategy<Value = Narrow> {
    prop_oneof![
        Just(Narrow::kilobits(100, Narrow::SCTP)),
        Just(Narrow::kilobits(300, Narrow::SCTP)),
        Just(Narrow::kilobits(1_000, 32 * 1024)),
        Just(Narrow::kilobits(5_000, 8 * 1024)),
    ]
}

/// The chaos stays cheap in a plain `cargo test`, and `PROPTEST_CASES`, as
/// the head of this file promises, searches further. A child of this test
/// binary runs the real property and the cases it played are counted.
#[test]
fn proptest_cases_really_widens_the_chaos() {
    let played = |asked: Option<&str>| {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child.args([
            "--exact",
            "chaos_never_loses_or_doubles_an_event",
            "--nocapture",
        ]);
        match asked {
            Some(cases) => child.env("PROPTEST_CASES", cases),
            None => child.env_remove("PROPTEST_CASES"),
        };
        let output = child.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.ends_with(CASE))
            .count()
    };
    // Proptest plays the saved failures first, on top of the cases asked.
    let saved = include_str!("properties.proptest-regressions")
        .lines()
        .filter(|line| line.starts_with("cc "))
        .count();
    assert_eq!(played(None), 64 + saved);
    assert_eq!(played(Some("80")), 80 + saved);
}
