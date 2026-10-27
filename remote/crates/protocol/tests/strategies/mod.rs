//! Random messages for the property tests, written by hand: every variant,
//! and the values a hostile or buggy game or phone would send (NaN, infinite
//! and huge sizes, ids past [`MAX_ID`], long and strange texts). Shared by
//! `remote-protocol`'s and `remote-core`'s tests.

use proptest::collection::vec;
use proptest::prelude::*;
use remote_protocol::{
    Action, Anchor, Control, Direction, DrawEvent, Feedback, GameMessage, Gate, Haptic, InputState,
    Kind, Label, Layout, MAX_ID, PhoneMessage, Place, Remote, Shape, StickValue,
};

/// A size or offset in points: mostly plausible, sometimes poisonous.
pub fn points() -> impl Strategy<Value = f32> {
    prop_oneof![
        6 => -400.0_f32..400.0,
        1 => any::<f32>(),
        1 => prop::sample::select(vec![
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            0.0,
            f32::MAX,
            f32::MIN,
            f32::MIN_POSITIVE,
            1e-45,
            -1.0,
            1e30,
        ]),
    ]
}

/// Texts from `tools/cert/nasty.json`, and anything else.
pub fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[a-zA-Z/?=.:]{0,12}",
        1 => prop::sample::select(vec![
            "👨‍👩‍👧‍👦",
            "🤦🏼‍♂️",
            "Z̷̢̛̖͎̗a̸̧͈͑l̶̢̛g̵o̷",
            "مرحبا",
            "שלום",
            "<img src=x onerror=alert(1)>",
            "{joueur}",
            "   ",
            "",
            "\u{0}",
        ])
        .prop_map(str::to_owned),
        1 => vec(any::<char>(), 0..300).prop_map(String::from_iter),
    ]
}

/// A control id: mostly valid, sometimes past [`MAX_ID`].
pub fn id() -> impl Strategy<Value = u8> {
    prop_oneof![6 => 0..=MAX_ID, 1 => any::<u8>()]
}

pub fn shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        points().prop_map(|radius| Shape::Circle { radius }),
        (points(), points()).prop_map(|(width, height)| Shape::Capsule { width, height }),
        (points(), points(), points()).prop_map(|(width, height, corner)| {
            Shape::RoundedRect {
                width,
                height,
                corner,
            }
        }),
        points().prop_map(|margin| Shape::Fill { margin }),
    ]
}

pub fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        any::<u32>().prop_map(Action::Color),
        any::<u16>().prop_map(Action::Size),
        Just(Action::Undo),
        Just(Action::Clear),
        Just(Action::Recenter),
    ]
}

pub fn kind() -> impl Strategy<Value = Kind> {
    prop_oneof![
        3 => (shape(), any::<bool>(), any::<bool>())
            .prop_map(|(shape, chord, minor)| Kind::Button { shape, chord, minor }),
        1 => (points(), points(), any::<bool>()).prop_map(|(travel, knob_radius, octagon)| {
            Kind::Stick {
                travel,
                knob_radius,
                gate: if octagon { Gate::Octagon } else { Gate::Round },
            }
        }),
        1 => points().prop_map(|size| Kind::DPad { size }),
        1 => (action(), shape()).prop_map(|(action, shape)| Kind::Action { action, shape }),
    ]
}

pub fn anchor() -> impl Strategy<Value = Anchor> {
    prop::sample::select(vec![
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ])
}

pub fn direction() -> impl Strategy<Value = Direction> {
    prop::sample::select(vec![
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ])
}

pub fn label() -> impl Strategy<Value = Label> {
    prop_oneof![
        text().prop_map(Label::Text),
        direction().prop_map(Label::Arrow),
        Just(Label::None),
    ]
}

pub fn control() -> impl Strategy<Value = Control> {
    (
        id(),
        kind(),
        anchor(),
        points(),
        points(),
        label(),
        any::<u32>(),
    )
        .prop_map(|(id, kind, anchor, x, y, label, color)| Control {
            id,
            kind,
            at: Place::new(anchor, x, y),
            label,
            color,
        })
}

pub fn layout() -> impl Strategy<Value = Layout> {
    (
        vec(control(), 0..16),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(controls, tilt, pointer, canvas)| Layout {
            controls,
            tilt,
            pointer,
            canvas,
        })
}

pub fn haptic() -> impl Strategy<Value = Haptic> {
    prop::sample::select(vec![
        Haptic::Press,
        Haptic::Release,
        Haptic::Success,
        Haptic::Warning,
    ])
}

pub fn game_message() -> impl Strategy<Value = GameMessage> {
    prop_oneof![
        3 => layout().prop_map(|layout| GameMessage::Show(Remote::Pad(layout))),
        1 => text().prop_map(|url| GameMessage::Show(Remote::Web { url })),
        1 => (prop::option::of(haptic()), prop::option::of(any::<u32>()))
            .prop_map(|(haptic, color)| GameMessage::Feedback(Feedback { haptic, color })),
        1 => text().prop_map(|reason| GameMessage::Refuse { reason }),
    ]
}

pub fn draw_event() -> impl Strategy<Value = DrawEvent> {
    prop_oneof![
        (
            any::<u16>(),
            any::<u32>(),
            any::<u16>(),
            any::<i16>(),
            any::<i16>()
        )
            .prop_map(|(stroke, color, width, x, y)| DrawEvent::Begin {
                stroke,
                color,
                width,
                x,
                y,
            }),
        (any::<u16>(), any::<i16>(), any::<i16>()).prop_map(|(stroke, x, y)| DrawEvent::Point {
            stroke,
            x,
            y
        }),
        any::<u16>().prop_map(|stroke| DrawEvent::End { stroke }),
        Just(DrawEvent::Undo),
        Just(DrawEvent::Clear),
    ]
}

pub fn input_state() -> impl Strategy<Value = InputState> {
    (
        any::<u32>(),
        any::<u32>(),
        vec(
            (any::<u8>(), any::<i8>(), any::<i8>()).prop_map(|(id, x, y)| StickValue { id, x, y }),
            0..4,
        ),
        any::<i8>(),
        prop::option::of(any::<[u16; 2]>()),
    )
        .prop_map(|(seq, pressed, sticks, tilt, pointer)| InputState {
            seq,
            pressed,
            sticks,
            tilt,
            pointer,
        })
}

pub fn phone_message() -> impl Strategy<Value = PhoneMessage> {
    prop_oneof![
        (any::<u16>(), any::<[u8; 16]>(), text()).prop_map(|(version, player, name)| {
            PhoneMessage::Hello {
                version,
                player,
                name,
            }
        }),
        input_state().prop_map(PhoneMessage::Input),
        draw_event().prop_map(PhoneMessage::Draw),
    ]
}

/// How bytes get damaged on their way.
#[derive(Clone, Debug)]
pub enum Damage {
    Truncate(prop::sample::Index),
    Flip(prop::sample::Index, u8),
    Insert(prop::sample::Index, u8),
    Remove(prop::sample::Index),
    /// A length or a count claiming more than the frame could ever hold.
    Huge(prop::sample::Index),
}

pub fn damage() -> impl Strategy<Value = Damage> {
    use prop::sample::Index;
    prop_oneof![
        any::<Index>().prop_map(Damage::Truncate),
        (any::<Index>(), 1..=u8::MAX).prop_map(|(at, bits)| Damage::Flip(at, bits)),
        (any::<Index>(), any::<u8>()).prop_map(|(at, byte)| Damage::Insert(at, byte)),
        any::<Index>().prop_map(Damage::Remove),
        any::<Index>().prop_map(Damage::Huge),
    ]
}

impl Damage {
    pub fn apply(&self, bytes: &mut Vec<u8>) {
        let at = |index: &prop::sample::Index, len: usize| index.index(len.max(1)).min(len);
        match self {
            Self::Truncate(index) => bytes.truncate(at(index, bytes.len())),
            Self::Flip(index, bits) => {
                let i = at(index, bytes.len());
                if let Some(byte) = bytes.get_mut(i) {
                    *byte ^= bits;
                }
            }
            Self::Insert(index, byte) => bytes.insert(at(index, bytes.len()), *byte),
            Self::Remove(index) => {
                let i = at(index, bytes.len());
                if i < bytes.len() {
                    bytes.remove(i);
                }
            }
            Self::Huge(index) => {
                let i = at(index, bytes.len());
                bytes.splice(i..i, HUGE_VARINT);
            }
        }
    }
}

/// `u64::MAX` as a postcard varint: as a length, more than any memory.
pub const HUGE_VARINT: [u8; 10] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
