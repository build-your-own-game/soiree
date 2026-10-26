//! Ready-made remotes. Each is a [`Layout`] like any other: read them to
//! compose your own. Sizes and spacing come from touch emulators, Apple's and
//! Microsoft's touch controls and the original pads; the sources are in the
//! remote's `LEARNINGS.md`.

use crate::layout::{Action, Anchor, Control, Direction, Gate, Kind, Label, Layout, Place, Shape};

fn button(id: u8, at: Place, shape: Shape, label: &str, color: u32) -> Control {
    Control {
        id,
        kind: Kind::Button {
            shape,
            chord: false,
            minor: false,
        },
        at,
        label: Label::Text(label.into()),
        color,
    }
}

/// A button a thumb presses together with its chord neighbours.
fn chord(id: u8, at: Place, radius: f32, label: &str, color: u32) -> Control {
    Control {
        kind: Kind::Button {
            shape: Shape::Circle { radius },
            chord: true,
            minor: false,
        },
        ..button(id, at, Shape::Circle { radius }, label, color)
    }
}

/// Start, Select: exactly their drawing, never hit by accident.
fn minor(id: u8, at: Place, shape: Shape, label: &str, color: u32) -> Control {
    Control {
        kind: Kind::Button {
            shape,
            chord: false,
            minor: true,
        },
        ..button(id, at, shape, label, color)
    }
}

fn stick(id: u8, at: Place, gate: Gate) -> Control {
    Control {
        id,
        kind: Kind::Stick {
            travel: 50.0,
            knob_radius: 30.0,
            gate,
        },
        at,
        label: Label::None,
        color: 0x00C7_C9CE,
    }
}

fn dpad(id: u8, at: Place, size: f32) -> Control {
    Control {
        id,
        kind: Kind::DPad { size },
        at,
        label: Label::None,
        color: 0x005C_5E66,
    }
}

fn action(id: u8, at: Place, action: Action, label: Label, color: u32) -> Control {
    Control {
        id,
        kind: Kind::Action {
            action,
            shape: Shape::Circle { radius: 23.0 },
        },
        at,
        label,
        color,
    }
}

fn place(anchor: Anchor, x: f32, y: f32) -> Place {
    Place::new(anchor, x, y)
}

/// The minimal action pad: a stick on the left half, A and B under the right
/// thumb, a small Start at the top.
pub mod pad {
    pub const A: u8 = 0;
    pub const B: u8 = 1;
    pub const START: u8 = 2;
    pub const STICK: u8 = 3;
}

pub fn pad() -> Layout {
    use Anchor::{BottomLeft, BottomRight, Top};
    Layout {
        controls: vec![
            // B down-left of A at 40°, 16 pt apart: a thumb on the seam
            // presses both.
            chord(
                pad::A,
                place(BottomRight, -110.0, -130.0),
                48.0,
                "A",
                0x002E_7CF6,
            ),
            chord(
                pad::B,
                place(BottomRight, -190.0, -63.0),
                40.0,
                "B",
                0x00F2_5F5C,
            ),
            minor(
                pad::START,
                place(Top, 0.0, 30.0),
                Shape::Circle { radius: 22.0 },
                "START",
                0x006B_6E76,
            ),
            stick(pad::STICK, place(BottomLeft, 150.0, -120.0), Gate::Round),
        ],
        ..Layout::default()
    }
}

/// A NES pad held sideways: a D-pad, A, B, Select and Start. For grids and
/// menus, where a stick is too loose.
pub mod cross {
    pub const UP: u8 = 0;
    pub const DOWN: u8 = 1;
    pub const LEFT: u8 = 2;
    pub const RIGHT: u8 = 3;
    pub const A: u8 = 4;
    pub const B: u8 = 5;
    pub const SELECT: u8 = 6;
    pub const START: u8 = 7;
}

pub fn cross() -> Layout {
    use Anchor::{Bottom, BottomLeft, BottomRight};
    let small = Shape::Capsule {
        width: 80.0,
        height: 32.0,
    };
    Layout {
        controls: vec![
            dpad(cross::UP, place(BottomLeft, 110.0, -140.0), 150.0),
            // B down-left of A at 45°, 20 pt apart, as on the NES layouts of
            // touch emulators.
            chord(
                cross::A,
                place(BottomRight, -80.0, -140.0),
                42.0,
                "A",
                0x00C8_102E,
            ),
            chord(
                cross::B,
                place(BottomRight, -153.5, -66.5),
                42.0,
                "B",
                0x00C8_102E,
            ),
            minor(
                cross::SELECT,
                place(Bottom, -50.0, -26.0),
                small,
                "SELECT",
                0x004A_4D55,
            ),
            minor(
                cross::START,
                place(Bottom, 50.0, -26.0),
                small,
                "START",
                0x004A_4D55,
            ),
        ],
        ..Layout::default()
    }
}

/// Tilt the phone like a steering wheel. Brake and item under the left thumb,
/// gas and drift under the right one, one per corner.
pub mod wheel {
    pub const GAS: u8 = 0;
    pub const BRAKE: u8 = 1;
    pub const DRIFT: u8 = 2;
    pub const ITEM: u8 = 3;
}

pub fn wheel() -> Layout {
    use Anchor::{BottomLeft, BottomRight, TopLeft, TopRight};
    let pedal = Shape::RoundedRect {
        width: 170.0,
        height: 190.0,
        corner: 28.0,
    };
    let side = Shape::Capsule {
        width: 150.0,
        height: 70.0,
    };
    Layout {
        controls: vec![
            button(
                wheel::GAS,
                place(BottomRight, -101.0, -111.0),
                pedal,
                "GAZ",
                0x002B_B673,
            ),
            button(
                wheel::BRAKE,
                place(BottomLeft, 101.0, -111.0),
                pedal,
                "FREIN",
                0x00E6_3946,
            ),
            button(
                wheel::DRIFT,
                place(TopRight, -91.0, 51.0),
                side,
                "DÉRAPAGE",
                0x0058_5D69,
            ),
            button(
                wheel::ITEM,
                place(TopLeft, 91.0, 51.0),
                side,
                "OBJET",
                0x00F4_A261,
            ),
        ],
        tilt: true,
        ..Layout::default()
    }
}

/// Aim the phone at the TV like a Wii Remote. A fires, B under the left
/// thumb, Center brings the cursor back.
pub mod pointer {
    pub const FIRE: u8 = 0;
    pub const B: u8 = 1;
    pub const CENTER: u8 = 2;
}

pub fn pointer() -> Layout {
    use Anchor::{BottomLeft, BottomRight, Top};
    let center = Shape::Capsule {
        width: 120.0,
        height: 44.0,
    };
    Layout {
        controls: vec![
            button(
                pointer::FIRE,
                place(BottomRight, -90.0, -90.0),
                Shape::Circle { radius: 60.0 },
                "A",
                0x003A_86FF,
            ),
            button(
                pointer::B,
                place(BottomLeft, 80.0, -80.0),
                Shape::Circle { radius: 48.0 },
                "B",
                0x0058_5D69,
            ),
            Control {
                id: pointer::CENTER,
                kind: Kind::Action {
                    action: Action::Recenter,
                    shape: center,
                },
                at: place(Top, 0.0, 26.0),
                label: Label::Text("CENTRER".into()),
                color: 0x003C_3F47,
            },
        ],
        pointer: true,
        ..Layout::default()
    }
}

/// The whole screen is one button.
pub mod buzzer {
    pub const BUZZ: u8 = 0;
}

pub fn buzzer() -> Layout {
    Layout {
        controls: vec![button(
            buzzer::BUZZ,
            place(Anchor::Center, 0.0, 0.0),
            Shape::Fill { margin: 12.0 },
            "BUZZ!",
            0x00E6_3946,
        )],
        ..Layout::default()
    }
}

/// A canvas, seven colors down the left side, three sizes, Undo and Clear
/// down the right.
pub mod draw {
    /// Colors take ids 0 to 6.
    pub const PALETTE: [u32; 7] = [
        0x001A_1A1A,
        0x00FF_FFFF,
        0x00E6_3946,
        0x00FF_D60A,
        0x002A_9D8F,
        0x003A_86FF,
        0x0083_38EC,
    ];
    /// Line widths in canvas units (0.5 %, 1 % and 2 % of its width), ids 7
    /// to 9.
    pub const SIZES: [u16; 3] = [20, 41, 82];
    pub const FIRST_SIZE: u8 = 7;
    pub const UNDO: u8 = 10;
    pub const CLEAR: u8 = 11;
}

pub fn draw() -> Layout {
    use Anchor::{Left, Right};
    let gray = 0x003C_3F47;
    // Columns centered on the screen's height, kept off its top edge, where
    // iOS holds touches back for its own gestures.
    let mut controls: Vec<Control> = (0u8..)
        .zip(draw::PALETTE)
        .map(|(id, color)| {
            action(
                id,
                place(Left, 32.0, f32::from(id) * 48.0 - 144.0),
                Action::Color(color),
                Label::None,
                color,
            )
        })
        .collect();
    let mut y = -120.0;
    for (id, width) in (draw::FIRST_SIZE..).zip(draw::SIZES) {
        controls.push(action(
            id,
            place(Right, -32.0, y),
            Action::Size(width),
            Label::None,
            gray,
        ));
        y += 60.0;
    }
    controls.push(action(
        draw::UNDO,
        place(Right, -32.0, y),
        Action::Undo,
        Label::Text("↶".into()),
        gray,
    ));
    controls.push(action(
        draw::CLEAR,
        place(Right, -32.0, y + 60.0),
        Action::Clear,
        Label::Text("✕".into()),
        gray,
    ));
    Layout {
        controls,
        canvas: true,
        ..Layout::default()
    }
}

/// The Nintendo 64 pad. The stick moves in the real pad's octagonal gate; Z
/// joins A and B on the right, since the index finger that held it on the
/// real pad has no button on a phone.
pub mod n64 {
    pub const A: u8 = 0;
    pub const B: u8 = 1;
    pub const Z: u8 = 2;
    pub const START: u8 = 3;
    /// Up; down, left and right follow.
    pub const D_UP: u8 = 4;
    pub const L: u8 = 8;
    pub const R: u8 = 9;
    pub const C_UP: u8 = 10;
    pub const C_DOWN: u8 = 11;
    pub const C_LEFT: u8 = 12;
    pub const C_RIGHT: u8 = 13;
    pub const STICK: u8 = 14;
}

pub fn n64() -> Layout {
    use Anchor::{BottomLeft, BottomRight, Top, TopLeft, TopRight};
    let shoulder = Shape::Capsule {
        width: 130.0,
        height: 46.0,
    };
    let gray = 0x008C_8F94;
    let yellow = 0x00FA_C71A;
    let c = |id, dx: f32, dy: f32, direction| Control {
        id,
        kind: Kind::Button {
            shape: Shape::Circle { radius: 23.0 },
            chord: false,
            minor: false,
        },
        at: place(TopRight, -78.0 + dx * 47.0, 140.0 + dy * 47.0),
        label: Label::Arrow(direction),
        color: yellow,
    };
    Layout {
        controls: vec![
            button(n64::L, place(TopLeft, 75.0, 29.0), shoulder, "L", gray),
            button(n64::R, place(TopRight, -75.0, 29.0), shoulder, "R", gray),
            minor(
                n64::START,
                place(Top, 0.0, 30.0),
                Shape::Circle { radius: 22.0 },
                "START",
                0x00DB_3029,
            ),
            // A, B and Z: a triangle with 20 pt between neighbours.
            chord(
                n64::A,
                place(BottomRight, -150.0, -70.0),
                40.0,
                "A",
                0x002A_5CD6,
            ),
            chord(
                n64::B,
                place(BottomRight, -240.0, -105.0),
                36.0,
                "B",
                0x0021_994C,
            ),
            chord(n64::Z, place(BottomRight, -165.0, -166.0), 34.0, "Z", gray),
            c(n64::C_UP, 0.0, -1.0, Direction::Up),
            c(n64::C_DOWN, 0.0, 1.0, Direction::Down),
            c(n64::C_LEFT, -1.0, 0.0, Direction::Left),
            c(n64::C_RIGHT, 1.0, 0.0, Direction::Right),
            dpad(n64::D_UP, place(TopLeft, 66.0, 112.0), 104.0),
            stick(n64::STICK, place(BottomLeft, 165.0, -115.0), Gate::Octagon),
        ],
        ..Layout::default()
    }
}

/// Every preset, for tests and demos.
pub fn all() -> [(&'static str, Layout); 7] {
    [
        ("pad", pad()),
        ("cross", cross()),
        ("wheel", wheel()),
        ("pointer", pointer()),
        ("buzzer", buzzer()),
        ("draw", draw()),
        ("n64", n64()),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::layout::MAX_ID;

    #[test]
    fn ids_are_unique_and_fit_in_the_pressed_bits() {
        for (name, layout) in all() {
            let mut seen = BTreeSet::new();
            for control in &layout.controls {
                let used = if matches!(control.kind, Kind::DPad { .. }) {
                    4
                } else {
                    1
                };
                for id in control.id..control.id + used {
                    assert!(id <= MAX_ID, "{name}: id {id} too large");
                    assert!(seen.insert(id), "{name}: id {id} used twice");
                }
            }
        }
    }

    #[test]
    fn labels_and_colors_can_be_changed() {
        let layout = pad()
            .label(pad::A, "Sauter")
            .color(pad::A, 0x0012_3456)
            .without(pad::START);
        let a = layout.controls.iter().find(|c| c.id == pad::A);
        assert_eq!(
            a.map(|c| (&c.label, c.color)),
            Some((&Label::Text("Sauter".into()), 0x0012_3456))
        );
        assert!(layout.controls.iter().all(|c| c.id != pad::START));
    }
}
