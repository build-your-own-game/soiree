//! Stage 1 of `CERTIFICATION.md` for the phone's side: whatever a game sends
//! (unreadable frames, hostile layouts) the session keeps working and only
//! trusts what is safe; whatever the fingers and the network do, the phone
//! says the same `Hello` and its states keep their rhythm and order.
//! `PROPTEST_CASES=5000 cargo test` searches further than the default 256
//! cases.

#[path = "../../protocol/tests/strategies/mod.rs"]
#[expect(
    dead_code,
    reason = "shared with remote-protocol's tests, which also generate phone messages"
)]
mod strategies;

use std::fmt::Write as _;

use proptest::collection::vec;
use proptest::prelude::*;
use remote_core::join::target;
use remote_core::remote::{Motion, Remote};
use remote_core::session::{Event, Session};
use remote_core::{Insets, Point, Size};
use remote_protocol::{
    DrawEvent, GameMessage, InputState, MAX_ID, PATH, PhoneMessage, Remote as Shown, VERSION,
    decode, encode, presets,
};
use strategies::{damage, game_message, layout, text};

/// Screens and system insets as UIKit reports them: iPhone 17 Pro in
/// landscape and in portrait, and an iPhone SE without a notch.
const SCREENS: [(Size, Insets); 3] = [
    (
        Size {
            width: 874.0,
            height: 402.0,
        },
        Insets {
            top: 0.0,
            left: 62.0,
            bottom: 21.0,
            right: 62.0,
        },
    ),
    (
        Size {
            width: 402.0,
            height: 874.0,
        },
        Insets {
            top: 62.0,
            left: 0.0,
            bottom: 34.0,
            right: 0.0,
        },
    ),
    (
        Size {
            width: 667.0,
            height: 375.0,
        },
        Insets {
            top: 0.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        },
    ),
];

fn screen() -> impl Strategy<Value = (Size, Insets)> {
    prop::sample::select(SCREENS.to_vec())
}

/// A finger position: on the screen or a little off it.
fn point() -> impl Strategy<Value = Point> {
    (-50.0_f32..950.0, -50.0_f32..950.0).prop_map(|(x, y)| Point::new(x, y))
}

/// What the player and the phone do between two frames.
#[derive(Clone, Debug)]
enum Hand {
    Down(u64, Point),
    Move(u64, Point),
    Up(u64),
    /// Tilted by an angle, turning at a rate, in landscape-left or right.
    Motion(f32, f32, bool),
    ReleaseAll,
}

fn hand() -> impl Strategy<Value = Hand> {
    prop_oneof![
        3 => (0..3_u64, point()).prop_map(|(id, p)| Hand::Down(id, p)),
        4 => (0..3_u64, point()).prop_map(|(id, p)| Hand::Move(id, p)),
        3 => (0..3_u64).prop_map(Hand::Up),
        1 => (-3.2_f32..3.2, -10.0_f32..10.0, any::<bool>())
            .prop_map(|(angle, rate, left)| Hand::Motion(angle, rate, left)),
        1 => Just(Hand::ReleaseAll),
    ]
}

/// Plays a hand on the session's remote, at `time`.
fn play(session: &mut Session, hand: &Hand, time: f64) {
    let Some(remote) = session.remote_mut() else {
        return;
    };
    match *hand {
        Hand::Down(id, p) => {
            remote.touch_down(id, p, time);
        }
        Hand::Move(id, p) => {
            remote.touch_move(id, p, time);
        }
        Hand::Up(id) => {
            remote.touch_up(id);
        }
        Hand::Motion(angle, rate, landscape_left) => remote.motion(Motion {
            gravity: [libm::cosf(angle), libm::sinf(angle), 0.0],
            rotation_rate: [rate, rate / 2.0, -rate],
            time,
            landscape_left,
        }),
        Hand::ReleaseAll => remote.release_all(),
    }
}

/// Every frame the session queued, which must all be phone messages.
fn sent(session: &mut Session) -> Vec<PhoneMessage> {
    let frames = session.drain();
    let messages: Vec<PhoneMessage> = frames.iter().filter_map(|f| decode(f).ok()).collect();
    assert_eq!(
        messages.len(),
        frames.len(),
        "the phone sent an unreadable frame"
    );
    messages
}

/// The bits of every id the placed layout can press.
fn pressable(session: &Session) -> u32 {
    let Some(remote) = session.remote() else {
        return 0;
    };
    let layout = remote.layout();
    let ids = layout
        .buttons
        .iter()
        .map(|b| b.id)
        .chain(layout.dpads.iter().flat_map(|d| d.ids));
    ids.fold(0, |bits, id| bits | 1 << id)
}

/// Every control of the placed layout is one the app can draw and press:
/// an id with a bit, a finite position, sizes finite and not negative.
fn assert_safe(session: &Session) {
    let Some(remote) = session.remote() else {
        return;
    };
    let layout = remote.layout();
    let finite = |p: Point| p.x.is_finite() && p.y.is_finite();
    for b in &layout.buttons {
        assert!(b.id <= MAX_ID && finite(b.center), "{b:?}");
        let bounds = b.shape.bounds(Point::default());
        assert!(bounds.width() >= 0.0 && bounds.height() >= 0.0, "{b:?}");
    }
    for d in &layout.dpads {
        assert!(d.ids.iter().all(|id| *id <= MAX_ID), "{d:?}");
        assert!(
            finite(d.center) && d.size.is_finite() && d.size > 0.0,
            "{d:?}"
        );
    }
    for s in &layout.sticks {
        assert!(s.id <= MAX_ID && finite(s.center), "{s:?}");
        assert!(s.travel.is_finite() && s.travel > 0.0, "{s:?}");
        assert!(s.knob_radius.is_finite() && s.knob_radius > 0.0, "{s:?}");
    }
}

/// A frame from the game: a real message, damaged or not, or noise.
fn game_frame() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        2 => game_message().prop_map(|message| encode(&message)),
        2 => (game_message(), vec(damage(), 1..3)).prop_map(|(message, damages)| {
            let mut frame = encode(&message);
            for damage in &damages {
                damage.apply(&mut frame);
            }
            frame
        }),
        1 => vec(any::<u8>(), 0..64),
    ]
}

proptest! {
    /// E1: a frame from the game that cannot be read is reported as such and
    /// changes nothing; a readable one is acted on. Through any mix of frames
    /// and fingers the session never panics, and everything it sends is a
    /// readable phone message.
    #[test]
    fn any_frame_from_the_game_is_acted_on_or_ignored(
        (screen, insets) in screen(),
        steps in vec((game_frame(), vec(hand(), 0..4)), 1..12),
    ) {
        let mut session = Session::new([9; 16], "Axel".into(), screen, insets);
        session.connected();
        let mut now = 0.0;
        for (frame, hands) in &steps {
            let shown = format!("{:?}", session.remote().map(Remote::spec));
            let event = session.receive(frame);
            match decode::<GameMessage>(frame) {
                Err(_) => {
                    prop_assert_eq!(&event, &Event::Unreadable);
                    prop_assert_eq!(format!("{:?}", session.remote().map(Remote::spec)), shown);
                }
                Ok(GameMessage::Show(Shown::Pad(_))) => prop_assert_eq!(&event, &Event::ShowPad),
                Ok(GameMessage::Show(Shown::Web { url })) => {
                    prop_assert_eq!(&event, &Event::ShowWeb(url));
                }
                Ok(GameMessage::Feedback(feedback)) => {
                    prop_assert_eq!(&event, &Event::Feedback(feedback));
                }
                Ok(GameMessage::Refuse { reason }) => {
                    prop_assert_eq!(&event, &Event::Refused(reason));
                }
            }
            assert_safe(&session);
            for hand in hands {
                now += 0.01;
                play(&mut session, hand, now);
                session.tick(now);
            }
            sent(&mut session);
        }
    }

    /// E1 and the validation `LEARNINGS.md` promises: a layout from a hostile
    /// game (ids past 31, NaN, infinite, negative or huge sizes and offsets)
    /// keeps only controls the app can draw and press. Fingers on it never
    /// panic, press only ids it holds, move only its sticks, and a lost hand
    /// lets go of everything.
    #[test]
    fn a_hostile_layout_keeps_only_safe_controls(
        (screen, insets) in screen(),
        spec in layout(),
        hands in vec(hand(), 0..40),
    ) {
        let mut session = Session::new([9; 16], "Axel".into(), screen, insets);
        let frame = encode(&GameMessage::Show(Shown::Pad(spec)));
        prop_assert_eq!(session.receive(&frame), Event::ShowPad);
        assert_safe(&session);
        let pressable = pressable(&session);
        let stick_ids: Vec<u8> = session
            .remote()
            .map(|r| r.layout().sticks.iter().map(|s| s.id).collect())
            .unwrap_or_default();
        let mut now = 0.0;
        for hand in &hands {
            now += 0.01;
            play(&mut session, hand, now);
            session.tick(now);
            for message in sent(&mut session) {
                if let PhoneMessage::Input(state) = message {
                    prop_assert_eq!(state.pressed & !pressable, 0, "pressed an unknown id");
                    prop_assert!(state.sticks.iter().all(|s| stick_ids.contains(&s.id)));
                }
            }
        }
        if let Some(remote) = session.remote_mut() {
            remote.release_all();
            prop_assert!(remote.pressed().is_empty());
        }
    }

    /// C2 and the contract's rhythm (`CONTRAT.md`, « Messages »): after every
    /// (re)connection the phone says the same `Hello`, with its player token
    /// and name; `seq` only goes up, across reconnections and remotes, so a
    /// game that ignores an older state never ignores a newer one; a state
    /// goes out at once on a new connection or remote, never twice within
    /// 1/60 s, and at least every 250 ms; drawing steps leave before the
    /// state of the same tick, and each stroke is a `Begin`, its points and
    /// an `End`, one stroke at a time.
    #[test]
    fn a_phone_comes_back_as_itself_and_keeps_its_rhythm(
        player in any::<[u8; 16]>(),
        name in text(),
        steps in vec(session_step(), 1..60),
    ) {
        let (screen, insets) = SCREENS[0];
        let mut session = Session::new(player, name.clone(), screen, insets);
        let hello = PhoneMessage::Hello { version: VERSION, player, name };
        let mut now = 0.0;
        let mut last_seq = None;
        // When the last state left, since the last connection or remote.
        let mut last_state: Option<f64> = None;
        // The stroke being drawn on the current canvas.
        let mut open = None;
        for step in &steps {
            match step {
                SessionStep::Connect => {
                    session.connected();
                    prop_assert_eq!(sent(&mut session), [hello.clone()]);
                    last_state = None;
                }
                SessionStep::Show(index) => {
                    let (_, spec) = presets::all()[*index % presets::all().len()].clone();
                    session.receive(&encode(&GameMessage::Show(Shown::Pad(spec))));
                    last_state = None;
                    open = None;
                }
                SessionStep::ShowWeb => {
                    let web = Shown::Web { url: "/free".into() };
                    session.receive(&encode(&GameMessage::Show(web)));
                    open = None;
                }
                SessionStep::Hand(hand) => play(&mut session, hand, now),
                SessionStep::Tick(dt) => {
                    now += dt;
                    let pad = session.remote().is_some();
                    session.tick(now);
                    let messages = sent(&mut session);
                    let states: Vec<&InputState> = messages
                        .iter()
                        .filter_map(|m| match m {
                            PhoneMessage::Input(state) => Some(state),
                            _ => None,
                        })
                        .collect();
                    prop_assert!(states.len() <= 1);
                    if let Some(state) = states.first() {
                        prop_assert!(last_seq.is_none_or(|last| state.seq > last), "seq went back");
                        last_seq = Some(state.seq);
                        prop_assert!(
                            matches!(messages.last(), Some(PhoneMessage::Input(_))),
                            "a drawing step left after the state"
                        );
                    }
                    let elapsed = last_state.map(|last| now - last);
                    if pad && elapsed.is_none_or(|e| e >= 0.25) {
                        prop_assert_eq!(states.len(), 1, "no state after {:?} s", elapsed);
                    }
                    if elapsed.is_some_and(|e| e < 1.0 / 60.0) {
                        prop_assert!(states.is_empty(), "two states within 1/60 s");
                    }
                    if !states.is_empty() {
                        last_state = Some(now);
                    }
                    for message in &messages {
                        if let PhoneMessage::Draw(event) = message {
                            follow_stroke(&mut open, *event);
                        }
                    }
                }
            }
        }
    }

    /// The QR code of any game page leads to its `/remote` socket, on the
    /// same host and port, with the same query; the app link carrying that
    /// page, percent-encoded, leads to the same place.
    #[test]
    fn a_game_page_and_its_app_link_lead_to_the_same_socket(
        secure in any::<bool>(),
        host in "[a-z0-9.-]{1,20}",
        port in prop::option::of(any::<u16>()),
        path in "(/[a-zA-Z0-9._~-]{0,8}){0,3}",
        query in prop::option::of("[a-zA-Z0-9=&%._~-]{1,20}"),
        fragment in prop::option::of("[a-z]{0,5}"),
    ) {
        let scheme = if secure { "https" } else { "http" };
        let authority = port.map_or(host.clone(), |port| format!("{host}:{port}"));
        let mut page = format!("{scheme}://{authority}{path}");
        if let Some(query) = &query {
            page = format!("{page}?{query}");
        }
        let without_fragment = page.clone();
        if let Some(fragment) = &fragment {
            page = format!("{page}#{fragment}");
        }
        let socket_scheme = if secure { "wss" } else { "ws" };
        let socket = match &query {
            Some(query) => format!("{socket_scheme}://{authority}{PATH}?{query}"),
            None => format!("{socket_scheme}://{authority}{PATH}"),
        };
        let found = target(&page).expect("a game page is a game");
        prop_assert_eq!(&found.socket, &socket);
        prop_assert_eq!(&found.page, &without_fragment);
        let encoded = page.bytes().fold(String::new(), |mut encoded, b| {
            let _ = write!(encoded, "%{b:02X}");
            encoded
        });
        prop_assert_eq!(target(&format!("telecommande://join?url={encoded}")), Some(found));
    }

    /// E1 for what the camera reads: any text is a game or not, never a
    /// panic, and a game's socket is always a WebSocket to `/remote`.
    #[test]
    fn any_scanned_text_is_a_game_or_not(scanned in prop_oneof![
        text(),
        text().prop_map(|t| format!("http://{t}")),
        text().prop_map(|t| format!("telecommande://join?url={t}")),
    ]) {
        if let Some(found) = target(&scanned) {
            prop_assert!(found.socket.starts_with("ws://") || found.socket.starts_with("wss://"));
            prop_assert!(found.socket.contains(PATH));
        }
    }
}

proptest! {
    /// E1: wherever a hostile layout puts a stick, and however small its
    /// travel, its knob is drawn at a finite point: the app hands it to
    /// UIKit, where a layer at NaN throws. Shrunk failure: a single stick
    /// `{ travel: 0.82, knob_radius: 1.2e-38 }` at `TopLeft` + `(f32::MAX, 0)`,
    /// which still owns its half of the screen; a thumb at `(484, 0)` puts its
    /// knob at `(NaN, 0)` (the offset over the travel overflows to infinity).
    #[test]
    #[ignore = "bug: a stick far off screen or with a tiny travel draws its knob at NaN (controls::TouchPad::knobs)"]
    fn a_hostile_layout_draws_its_knobs_at_finite_points(
        (screen, insets) in screen(),
        spec in layout(),
        hands in vec(hand(), 0..40),
    ) {
        let mut session = Session::new([9; 16], "Axel".into(), screen, insets);
        session.receive(&encode(&GameMessage::Show(Shown::Pad(spec))));
        let mut now = 0.0;
        for hand in &hands {
            now += 0.01;
            play(&mut session, hand, now);
            if let Some(remote) = session.remote() {
                for (id, knob) in remote.knobs() {
                    prop_assert!(knob.x.is_finite() && knob.y.is_finite(), "knob {} at {:?}", id, knob);
                }
            }
        }
    }
}

/// Checks that a drawing step continues the stroke stream: a stroke begins
/// when none is open, and its points and end name it.
fn follow_stroke(open: &mut Option<u16>, event: DrawEvent) {
    match event {
        DrawEvent::Begin { stroke, .. } => {
            assert_eq!(
                *open, None,
                "stroke {stroke} began before the last one ended"
            );
            *open = Some(stroke);
        }
        DrawEvent::Point { stroke, .. } => assert_eq!(*open, Some(stroke), "a stray point"),
        DrawEvent::End { stroke } => {
            assert_eq!(open.take(), Some(stroke), "a stray end");
        }
        DrawEvent::Undo | DrawEvent::Clear => {}
    }
}

/// `CONTRAT.md`: the strokes of a canvas « arrivent dans l'ordre » and the
/// socket « les livre tous ». A game that changes the remote while a finger
/// draws must still get that stroke's end, as `Session::receive` intends by
/// releasing the old remote; but the `End` it produces stays in the old
/// canvas, which is dropped, along with any step not yet sent.
#[test]
#[ignore = "bug: changing the remote mid-stroke drops the old canvas's unsent steps, End included (Session::receive)"]
fn a_stroke_cut_by_a_new_remote_still_ends() {
    let (screen, insets) = SCREENS[0];
    let mut session = Session::new([9; 16], "Axel".into(), screen, insets);
    session.receive(&encode(&GameMessage::Show(Shown::Pad(presets::draw()))));
    let rect = session
        .remote()
        .and_then(Remote::canvas_rect)
        .unwrap_or_default();
    let middle = Point::new(rect.center_x(), rect.center_y());
    if let Some(remote) = session.remote_mut() {
        remote.touch_down(1, middle, 0.0);
    }
    session.tick(0.0);
    if let Some(remote) = session.remote_mut() {
        remote.touch_move(1, Point::new(middle.x + 30.0, middle.y), 0.01);
    }
    session.receive(&encode(&GameMessage::Show(Shown::Pad(presets::pad()))));
    session.tick(0.02);
    let mut open = None;
    for message in sent(&mut session) {
        if let PhoneMessage::Draw(event) = message {
            follow_stroke(&mut open, event);
        }
    }
    assert_eq!(open, None, "the stroke never ended");
}

#[derive(Clone, Debug)]
enum SessionStep {
    Connect,
    Show(usize),
    ShowWeb,
    Hand(Hand),
    Tick(f64),
}

fn session_step() -> impl Strategy<Value = SessionStep> {
    prop_oneof![
        1 => Just(SessionStep::Connect),
        1 => (0..7_usize).prop_map(SessionStep::Show),
        1 => Just(SessionStep::ShowWeb),
        4 => hand().prop_map(SessionStep::Hand),
        4 => prop_oneof![
            (0.0_f64..0.02),
            (0.0_f64..0.4),
            Just(1.0 / 60.0),
            Just(0.25),
        ]
        .prop_map(SessionStep::Tick),
    ]
}
