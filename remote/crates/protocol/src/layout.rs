//! A remote the app draws itself: controls placed from the screen's anchors.
//! The app brings the feel (reach beyond the drawing, sliding between
//! buttons, a stick under the thumb, haptics); the layout says what and where.

use serde::{Deserialize, Serialize};

/// Ids go up to 31: one bit each in [`crate::InputState::pressed`].
pub const MAX_ID: u8 = 31;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub controls: Vec<Control>,
    /// Tilting the phone like a wheel steers: [`crate::InputState::tilt`].
    pub tilt: bool,
    /// Aiming the phone at the TV moves a cursor, and so does a finger
    /// dragged off the controls: [`crate::InputState::pointer`].
    pub pointer: bool,
    /// A 16:9 drawing canvas fills the space between the left and right
    /// 64 points of the screen; strokes arrive as [`crate::DrawEvent`].
    pub canvas: bool,
}

impl Layout {
    /// Renames a control.
    #[must_use]
    pub fn label(mut self, id: u8, text: impl Into<String>) -> Self {
        let text = text.into();
        for control in self.controls.iter_mut().filter(|c| c.id == id) {
            control.label = Label::Text(text.clone());
        }
        self
    }

    /// Recolors a control, `0xRRGGBB`.
    #[must_use]
    pub fn color(mut self, id: u8, color: u32) -> Self {
        for control in self.controls.iter_mut().filter(|c| c.id == id) {
            control.color = color;
        }
        self
    }

    /// Adds a control.
    #[must_use]
    pub fn with(mut self, control: Control) -> Self {
        self.controls.push(control);
        self
    }

    /// Removes a control.
    #[must_use]
    pub fn without(mut self, id: u8) -> Self {
        self.controls.retain(|c| c.id != id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Control {
    /// 0 to [`MAX_ID`]. A D-pad uses four: `id` (up) to `id + 3`.
    pub id: u8,
    pub kind: Kind,
    pub at: Place,
    pub label: Label,
    /// `0xRRGGBB`.
    pub color: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    /// Pressed while a finger is on it.
    Button {
        shape: Shape,
        /// A finger between two chord buttons presses both, as one thumb
        /// does between A and B.
        chord: bool,
        /// No reach beyond its drawing, so it is never hit by accident
        /// (Start, Select).
        minor: bool,
    },
    /// A thumb landing anywhere on the half of the screen the stick sits in
    /// takes it. Its value is in [`crate::InputState::sticks`].
    Stick {
        /// Knob travel to full deflection, in points.
        travel: f32,
        knob_radius: f32,
        gate: Gate,
    },
    /// Four directions, ids `id` (up), `id + 1` (down), `id + 2` (left),
    /// `id + 3` (right). Straight directions win over diagonals.
    DPad { size: f32 },
    /// A button the app acts on itself; it is also reported as pressed.
    Action { action: Action, shape: Shape },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Draw with this color, `0xRRGGBB`.
    Color(u32),
    /// Draw this wide, in canvas units.
    Size(u16),
    /// Remove the last stroke.
    Undo,
    /// Clear the canvas; a second tap within 2 seconds confirms.
    Clear,
    /// Bring the pointer back to the middle of the TV.
    Recenter,
}

/// A point `x`, `y` points away from an anchor of the usable screen (inside
/// the notch and home indicator): x to the right, y down. A control hangs from
/// the corner nearest the thumb that uses it, so a larger phone only widens
/// the middle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub anchor: Anchor,
    pub x: f32,
    pub y: f32,
}

impl Place {
    pub const fn new(anchor: Anchor, x: f32, y: f32) -> Self {
        Self { anchor, x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Circle {
        radius: f32,
    },
    /// A horizontal pill.
    Capsule {
        width: f32,
        height: f32,
    },
    RoundedRect {
        width: f32,
        height: f32,
        corner: f32,
    },
    /// The whole usable screen but a margin: the buzzer.
    Fill {
        margin: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gate {
    Round,
    /// The N64's octagon: 85 along an axis, 70 on both axes in a diagonal
    /// notch, scaled to 1 at full reach along an axis.
    Octagon,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Label {
    Text(String),
    Arrow(Direction),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
