//! The remote's brain, shared by the iPhone app and, later, fake phones.
//!
//! A preset computes its own layout for a screen size, takes raw touches
//! (an id and a position in points, origin top-left) and keeps the pad state.
//! The app only draws what the layout says and plays the haptics returned.
//! Everything here is plain arithmetic, so it is tested natively.

pub mod n64;

/// A position or offset in screen points, origin top-left, Y down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn distance(self, other: Self) -> f32 {
        self.minus(other).length()
    }
}

/// Screen size in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Areas the system keeps for itself (notch, home indicator), in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
}

/// The screen rectangle a preset may use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn usable(screen: Size, insets: Insets) -> Self {
        Self {
            left: insets.left,
            top: insets.top,
            right: screen.width - insets.right,
            bottom: screen.height - insets.bottom,
        }
    }

    pub fn height(self) -> f32 {
        self.bottom - self.top
    }

    pub fn center_x(self) -> f32 {
        self.left.midpoint(self.right)
    }

    pub fn contains_rect(self, other: Self) -> bool {
        other.left >= self.left
            && other.right <= self.right
            && other.top >= self.top
            && other.bottom <= self.bottom
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

/// A haptic the app plays right away, so a press is felt, not only seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Haptic {
    /// The stick reached its rim.
    Edge,
    /// A button came up: a lighter tick than a press.
    Release,
    /// A button went down.
    Press,
}
