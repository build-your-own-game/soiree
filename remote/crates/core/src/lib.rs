//! The remote's brain, shared by the iPhone app and, later, fake phones.
//!
//! A preset computes its own layout for a screen size, takes raw touches
//! (an id and a position in points, origin top-left) and keeps the pad state.
//! The app only draws what the layout says and plays the haptics returned.
//! Everything here is plain arithmetic, so it is tested natively.

pub mod controls;
pub mod draw;
#[cfg(test)]
mod feel;
pub mod join;
pub mod pointer;
pub mod remote;
pub mod session;
pub mod wheel;

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

    #[must_use]
    pub fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    #[must_use]
    pub fn plus(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn distance(self, other: Self) -> f32 {
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

    pub fn width(self) -> f32 {
        self.right - self.left
    }

    pub fn height(self) -> f32 {
        self.bottom - self.top
    }

    pub fn center_y(self) -> f32 {
        self.top.midpoint(self.bottom)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
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

#[cfg(test)]
mod tests {
    use super::{Insets, Point, Rect, Size};
    use crate::controls::Layout;

    /// Landscape iPhones: `(screen, insets)` as UIKit reports them.
    pub const PHONES: [(Size, Insets); 4] = [
        // iPhone 17 Pro
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
        // iPhone 17 Pro Max
        (
            Size {
                width: 956.0,
                height: 440.0,
            },
            Insets {
                top: 0.0,
                left: 62.0,
                bottom: 21.0,
                right: 62.0,
            },
        ),
        // iPhone SE (3rd generation), no notch
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
        // iPhone 17e
        (
            Size {
                width: 844.0,
                height: 390.0,
            },
            Insets {
                top: 0.0,
                left: 47.0,
                bottom: 21.0,
                right: 47.0,
            },
        ),
    ];

    pub fn circle_meets_rect(center: Point, radius: f32, rect: Rect) -> bool {
        let nearest = Point::new(
            center.x.clamp(rect.left, rect.right),
            center.y.clamp(rect.top, rect.bottom),
        );
        nearest.distance(center) < radius
    }

    /// Every control inside the usable area, none drawn over another, the
    /// sticks at rest clear of them all, and Apple's minimum sizes: 44 pt for
    /// play buttons, 28 pt for minor ones.
    pub fn assert_fits(layout: &Layout) {
        let drawn = layout.drawn();
        for (i, (name, rect)) in drawn.iter().enumerate() {
            assert!(
                layout.area.contains_rect(*rect),
                "{name} leaves the usable area"
            );
            for (other, other_rect) in drawn.iter().skip(i + 1) {
                if name == other {
                    continue;
                }
                // Two round buttons on a diagonal: their boxes overlap, the
                // circles must not.
                if let (Some((a, ra)), Some((b, rb))) =
                    (circle(layout, name), circle(layout, other))
                {
                    assert!(a.distance(b) >= ra + rb, "{name} overlaps {other}");
                } else {
                    assert!(!rect.overlaps(*other_rect), "{name} overlaps {other}");
                }
            }
        }
        for stick in &layout.sticks {
            let rest = crate::controls::Shape::Circle {
                radius: stick.reach(),
            }
            .bounds(stick.center);
            assert!(
                layout.area.contains_rect(rest),
                "stick {} leaves the usable area",
                stick.id
            );
            for (name, rect) in &drawn {
                assert!(
                    !circle_meets_rect(stick.center, stick.reach(), *rect),
                    "stick {} covers {name}",
                    stick.id
                );
            }
        }
        for b in &layout.buttons {
            let bounds = b.shape.bounds(b.center);
            let minimum = if b.slop > 0.0 { 44.0 } else { 28.0 };
            assert!(
                bounds.width().min(bounds.height()) >= minimum,
                "button {} is too small",
                b.id
            );
        }
    }

    fn circle(layout: &Layout, name: &str) -> Option<(Point, f32)> {
        layout
            .buttons
            .iter()
            .find(|b| format!("button {}", b.id) == name)
            .and_then(|b| match b.shape {
                crate::controls::Shape::Circle { radius } => Some((b.center, radius)),
                _ => None,
            })
    }
}
