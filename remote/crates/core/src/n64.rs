//! Nintendo 64 pad, held sideways. The left thumb has the stick and the
//! D-pad, the right thumb A, B, Z and the C buttons, and L, Start and R sit
//! along the top. Each choice and its source is in `LEARNINGS.md`. In short:
//! - a finger owns what it first touched until it lifts, so a thumb that
//!   drifts off the stick keeps steering and never presses a button;
//! - the whole left half takes the stick. A thumb landing near its rest
//!   position becomes its center; further out, the center stops on a small
//!   circle, so the stick already leans toward where the thumb landed;
//! - the stick moves in the real pad's octagonal gate;
//! - Z joins A and B on the right: on the real pad the index finger holds Z
//!   while the left thumb steers, and a phone only has two thumbs;
//! - a finger between two of A, B and Z presses both; elsewhere the nearest
//!   button wins, and a held button keeps a wider margin so it does not
//!   flicker on its edge;
//! - sizes are in points: a thumb has the same size on every iPhone.

use std::collections::BTreeMap;

use remote_protocol::{N64Button, N64Buttons, N64Pad};

use crate::{Haptic, Insets, Point, Rect, Size};

/// Reach added around each button and the D-pad, in points.
const SLOP: f32 = 15.0;
/// Reach around a button the finger already holds.
const HOLD_SLOP: f32 = 30.0;
/// Buttons pressed together with one thumb.
const CHORD: [N64Button; 3] = [N64Button::A, N64Button::B, N64Button::Z];
/// Stick deflection ignored around the center, as a fraction of the travel:
/// only a filter for finger jitter. Games add their own dead zone on top
/// (7 raw units in Super Mario 64 and Ocarina of Time, 5 in GoldenEye, 12 in
/// Mario Kart 64), so a bigger one here would stack with theirs.
const STICK_DEAD_ZONE: f32 = 0.03;
/// Radius around the stick's rest position, in travel units, in which a
/// landing thumb becomes the stick's center.
const RECENTER_ZONE: f32 = 0.6;
/// The rim haptic fires at this deflection, and re-arms below the next one.
const EDGE_ON: f32 = 0.97;
const EDGE_OFF: f32 = 0.85;
/// D-pad dead zone around its center, as a fraction of its size.
const DPAD_DEAD_ZONE: f32 = 0.08;
/// tan(25°): diagonals get 40° slices and the four axes 50°, so a thumb
/// aiming straight is not read as a diagonal.
const TAN_25: f32 = 0.466_307_66;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle {
        radius: f32,
    },
    /// A horizontal pill.
    Capsule {
        width: f32,
        height: f32,
    },
}

impl Shape {
    pub fn bounds(self, center: Point) -> Rect {
        let (half_width, half_height) = match self {
            Self::Circle { radius } => (radius, radius),
            Self::Capsule { width, height } => (width / 2.0, height / 2.0),
        };
        Rect {
            left: center.x - half_width,
            top: center.y - half_height,
            right: center.x + half_width,
            bottom: center.y + half_height,
        }
    }

    /// Distance from `point` to the shape's edge, negative inside.
    fn signed_distance(self, center: Point, point: Point) -> f32 {
        match self {
            Self::Circle { radius } => center.distance(point) - radius,
            Self::Capsule { width, height } => {
                let radius = height / 2.0;
                let half_segment = (width - height) / 2.0;
                let offset = point.minus(center);
                let along = (offset.x.abs() - half_segment).max(0.0);
                Point::new(along, offset.y).length() - radius
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonLayout {
    pub button: N64Button,
    pub center: Point,
    pub shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickLayout {
    /// Where the stick rests when no thumb holds it.
    pub center: Point,
    /// How far the knob's center travels to reach full deflection along an
    /// axis; along a diagonal it goes [`gate_diagonal_reach`] times further.
    pub travel: f32,
    pub knob_radius: f32,
}

impl StickLayout {
    /// Room the drawn stick needs around its center: the gate's diagonal
    /// corners plus the knob.
    pub fn reach(self) -> f32 {
        self.travel * gate_diagonal_reach() + self.knob_radius
    }

    /// How far the stick's center can move from its rest position.
    pub fn recenter_radius(self) -> f32 {
        RECENTER_ZONE * self.travel
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DPadLayout {
    pub center: Point,
    /// Width and height of the cross; each arm is a third of it wide.
    pub size: f32,
}

impl DPadLayout {
    /// The cross as its horizontal and vertical bars.
    pub fn bars(self) -> [Rect; 2] {
        let (half, arm) = (self.size / 2.0, self.size / 6.0);
        let (x, y) = (self.center.x, self.center.y);
        [
            Rect {
                left: x - half,
                top: y - arm,
                right: x + half,
                bottom: y + arm,
            },
            Rect {
                left: x - arm,
                top: y - half,
                right: x + arm,
                bottom: y + half,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The rectangle the controls fit in, outside the system's areas.
    pub area: Rect,
    pub stick: StickLayout,
    pub dpad: DPadLayout,
    pub buttons: Vec<ButtonLayout>,
}

impl Layout {
    /// Positions hang from the corners of the usable area, where the thumbs
    /// are, so a larger iPhone only widens the middle.
    pub fn new(screen: Size, insets: Insets) -> Self {
        let area = Rect::usable(screen, insets);
        let (left, top, right, bottom) = (area.left, area.top, area.right, area.bottom);
        let shoulder = Shape::Capsule {
            width: 130.0,
            height: 46.0,
        };
        let circle = |radius| Shape::Circle { radius };
        let button = |button, x, y, shape| ButtonLayout {
            button,
            center: Point::new(x, y),
            shape,
        };
        let (c_x, c_y, c_offset) = (right - 78.0, top + 140.0, 47.0);

        let buttons = vec![
            button(N64Button::L, left + 75.0, top + 29.0, shoulder),
            button(N64Button::R, right - 75.0, top + 29.0, shoulder),
            button(N64Button::Start, area.center_x(), top + 30.0, circle(22.0)),
            // A, B and Z: a triangle with 20 pt between neighbours, so their
            // reaches overlap where one thumb presses two.
            button(N64Button::A, right - 150.0, bottom - 70.0, circle(40.0)),
            button(N64Button::B, right - 240.0, bottom - 105.0, circle(36.0)),
            button(N64Button::Z, right - 165.0, bottom - 166.0, circle(34.0)),
            button(N64Button::CUp, c_x, c_y - c_offset, circle(23.0)),
            button(N64Button::CDown, c_x, c_y + c_offset, circle(23.0)),
            button(N64Button::CLeft, c_x - c_offset, c_y, circle(23.0)),
            button(N64Button::CRight, c_x + c_offset, c_y, circle(23.0)),
        ];

        Self {
            area,
            stick: StickLayout {
                center: Point::new(left + 165.0, bottom - 115.0),
                travel: 50.0,
                knob_radius: 30.0,
            },
            dpad: DPadLayout {
                center: Point::new(left + 66.0, top + 112.0),
                size: 104.0,
            },
            buttons,
        }
    }

    /// The buttons under `point` for a finger already holding `held`.
    fn buttons_at(&self, point: Point, held: N64Buttons) -> N64Buttons {
        let reach = |b: &ButtonLayout| {
            if held.contains(b.button) {
                HOLD_SLOP
            } else {
                SLOP
            }
        };
        let within: Vec<(f32, N64Button)> = self
            .buttons
            .iter()
            .map(|b| (b.shape.signed_distance(b.center, point), b.button, reach(b)))
            .filter(|(distance, _, reach)| distance <= reach)
            .map(|(distance, button, _)| (distance, button))
            .collect();
        let mut pressed = N64Buttons::default();
        let Some(&(_, nearest)) = within.iter().min_by(|a, b| a.0.total_cmp(&b.0)) else {
            return pressed;
        };
        if CHORD.contains(&nearest) {
            for (_, button) in within.iter().filter(|(_, b)| CHORD.contains(b)) {
                pressed.set(*button, true);
            }
        } else {
            pressed.set(nearest, true);
        }
        pressed
    }

    fn on_dpad(&self, point: Point) -> bool {
        let reach = self.dpad.size / 2.0 + SLOP;
        let offset = point.minus(self.dpad.center);
        offset.x.abs() <= reach && offset.y.abs() <= reach
    }

    fn on_stick_side(&self, point: Point) -> bool {
        point.x < self.area.center_x()
    }

    /// Where a stick taken at `point` is centered: there if the thumb landed
    /// close to the rest position, else on the recentering circle toward it.
    fn stick_origin(&self, point: Point) -> Point {
        let offset = point.minus(self.stick.center);
        let (length, zone) = (offset.length(), self.stick.recenter_radius());
        if length <= zone {
            point
        } else {
            Point::new(
                self.stick.center.x + offset.x * zone / length,
                self.stick.center.y + offset.y * zone / length,
            )
        }
    }
}

/// What a finger is holding, from touch-down to lift.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Capture {
    Stick {
        origin: Point,
        position: Point,
    },
    DPad {
        position: Point,
    },
    /// A finger on the buttons; none while it slides between them.
    Buttons {
        held: N64Buttons,
    },
}

/// Where to draw the stick's knob. Its gate stays drawn at rest: the hidden
/// recentering under the thumb must not look like the stick jumped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickView {
    pub knob: Point,
}

#[derive(Clone, Debug)]
pub struct N64Remote {
    layout: Layout,
    touches: BTreeMap<u64, Capture>,
    edge_armed: bool,
}

impl N64Remote {
    pub fn new(screen: Size, insets: Insets) -> Self {
        Self {
            layout: Layout::new(screen, insets),
            touches: BTreeMap::new(),
            edge_armed: true,
        }
    }

    /// A new screen size (rotation, split view) lays the pad out again and
    /// lets go of everything, like a lifted hand.
    pub fn resize(&mut self, screen: Size, insets: Insets) {
        self.layout = Layout::new(screen, insets);
        self.release_all();
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn touch_down(&mut self, id: u64, point: Point) -> Option<Haptic> {
        let before = self.pad();
        if let Some(capture) = self.capture_at(point) {
            self.touches.insert(id, capture);
        }
        self.feedback(before)
    }

    pub fn touch_move(&mut self, id: u64, point: Point) -> Option<Haptic> {
        let before = self.pad();
        let capture = self.touches.get(&id).copied()?;
        let moved = match capture {
            Capture::Stick { origin, .. } => Capture::Stick {
                origin,
                position: point,
            },
            Capture::DPad { .. } => Capture::DPad { position: point },
            Capture::Buttons { held } => Capture::Buttons {
                held: self.layout.buttons_at(point, held),
            },
        };
        self.touches.insert(id, moved);
        self.feedback(before)
    }

    pub fn touch_up(&mut self, id: u64) -> Option<Haptic> {
        let before = self.pad();
        self.touches.remove(&id);
        self.feedback(before)
    }

    /// The app lost its touches (app switcher, phone call): nothing may stay
    /// held.
    pub fn release_all(&mut self) {
        self.touches.clear();
        self.edge_armed = true;
    }

    /// The pad as a game would read it, from every finger at once.
    pub fn pad(&self) -> N64Pad {
        let mut pad = N64Pad::default();
        for capture in self.touches.values() {
            match *capture {
                Capture::Stick { origin, position } => {
                    (pad.stick_x, pad.stick_y) =
                        stick_value(position.minus(origin), self.layout.stick.travel);
                }
                Capture::DPad { position } => {
                    dpad_buttons(
                        &mut pad.buttons,
                        position.minus(self.layout.dpad.center),
                        self.layout.dpad.size,
                    );
                }
                Capture::Buttons { held } => pad.buttons.0 |= held.0,
            }
        }
        pad
    }

    pub fn stick(&self) -> StickView {
        let stick = self.layout.stick;
        match self.stick_capture() {
            Some((origin, position)) => {
                let offset = position.minus(origin);
                let knob = gate_clamp(Point::new(offset.x / stick.travel, offset.y / stick.travel));
                StickView {
                    knob: Point::new(
                        stick.center.x + knob.x * stick.travel,
                        stick.center.y + knob.y * stick.travel,
                    ),
                }
            }
            None => StickView { knob: stick.center },
        }
    }

    fn capture_at(&self, point: Point) -> Option<Capture> {
        let held = self.layout.buttons_at(point, N64Buttons::default());
        if held != N64Buttons::default() {
            return Some(Capture::Buttons { held });
        }
        if self.layout.on_dpad(point) {
            return Some(Capture::DPad { position: point });
        }
        if self.layout.on_stick_side(point) && self.stick_capture().is_none() {
            return Some(Capture::Stick {
                origin: self.layout.stick_origin(point),
                position: point,
            });
        }
        None
    }

    fn stick_capture(&self) -> Option<(Point, Point)> {
        self.touches.values().find_map(|capture| match *capture {
            Capture::Stick { origin, position } => Some((origin, position)),
            _ => None,
        })
    }

    /// Stick deflection from 0 (center) to 1 (gate) and beyond.
    fn deflection(&self) -> f32 {
        self.stick_capture().map_or(0.0, |(origin, position)| {
            let offset = position.minus(origin);
            let length = offset.length();
            if length == 0.0 {
                return 0.0;
            }
            length / self.layout.stick.travel / gate_reach(offset.x / length, offset.y / length)
        })
    }

    fn update_edge(&mut self) -> bool {
        let deflection = self.deflection();
        if self.edge_armed && deflection >= EDGE_ON {
            self.edge_armed = false;
            return true;
        }
        if deflection < EDGE_OFF {
            self.edge_armed = true;
        }
        false
    }

    fn feedback(&mut self, before: N64Pad) -> Option<Haptic> {
        let edge = self.update_edge();
        let after = self.pad().buttons.0;
        if after & !before.buttons.0 != 0 {
            Some(Haptic::Press)
        } else if before.buttons.0 & !after != 0 {
            Some(Haptic::Release)
        } else if edge {
            Some(Haptic::Edge)
        } else {
            None
        }
    }
}

/// Stick offset in points (Y down) to the pad's values (Y up). The thumb
/// moves the knob in the real pad's octagonal gate: `travel` points reach 85
/// along an axis, and a diagonal notch reads 70 on both axes. A round gate
/// would read 60 there and slow diagonal moves in games that read each axis
/// on its own (GoldenEye, Ocarina of Time).
fn stick_value(offset: Point, travel: f32) -> (i8, i8) {
    let knob = gate_clamp(Point::new(offset.x / travel, -offset.y / travel));
    let length = knob.length();
    if length <= STICK_DEAD_ZONE {
        return (0, 0);
    }
    let scale = (length - STICK_DEAD_ZONE) / (1.0 - STICK_DEAD_ZONE) / length
        * f32::from(N64Pad::STICK_CARDINAL);
    (to_axis(knob.x * scale), to_axis(knob.y * scale))
}

/// `offset` in travel units (1 = full along an axis), stopped at the gate.
fn gate_clamp(offset: Point) -> Point {
    let length = offset.length();
    if length == 0.0 {
        return offset;
    }
    let reach = gate_reach(offset.x / length, offset.y / length);
    if length <= reach {
        offset
    } else {
        Point::new(offset.x * reach / length, offset.y * reach / length)
    }
}

/// Distance from the center to the gate along a unit direction, in travel
/// units: 1 along an axis, 70/85·√2 ≈ 1.16 along a diagonal.
fn gate_reach(x: f32, y: f32) -> f32 {
    let diagonal = f32::from(N64Pad::STICK_DIAGONAL) / f32::from(N64Pad::STICK_CARDINAL);
    let (major, minor) = (x.abs().max(y.abs()), x.abs().min(y.abs()));
    // The gate edge from (1, 0) to (diagonal, diagonal), whose outward normal
    // is (diagonal, 1 - diagonal).
    diagonal / (diagonal * major + (1.0 - diagonal) * minor)
}

/// How far a stick reaches along a diagonal, relative to an axis.
pub fn gate_diagonal_reach() -> f32 {
    gate_reach(
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    )
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "rounded and clamped to ±STICK_CARDINAL first"
)]
fn to_axis(value: f32) -> i8 {
    let max = f32::from(N64Pad::STICK_CARDINAL);
    value.round().clamp(-max, max) as i8
}

/// The D-pad keeps its direction wherever the finger goes, until it lifts.
fn dpad_buttons(buttons: &mut N64Buttons, offset: Point, size: f32) {
    if offset.length() < DPAD_DEAD_ZONE * size {
        return;
    }
    if offset.x.abs() > TAN_25 * offset.y.abs() {
        buttons.set(
            if offset.x > 0.0 {
                N64Button::DRight
            } else {
                N64Button::DLeft
            },
            true,
        );
    }
    if offset.y.abs() > TAN_25 * offset.x.abs() {
        buttons.set(
            if offset.y > 0.0 {
                N64Button::DDown
            } else {
                N64Button::DUp
            },
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Landscape iPhones: `(screen, insets)` as UIKit reports them.
    const PHONES: [(Size, Insets); 4] = [
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

    fn remote() -> N64Remote {
        let (screen, insets) = PHONES[0];
        N64Remote::new(screen, insets)
    }

    fn center_of(remote: &N64Remote, button: N64Button) -> Point {
        remote
            .layout()
            .buttons
            .iter()
            .find(|b| b.button == button)
            .map(|b| b.center)
            .expect("button in layout")
    }

    fn held(remote: &N64Remote) -> Vec<N64Button> {
        N64Button::ALL
            .into_iter()
            .filter(|b| remote.pad().buttons.contains(*b))
            .collect()
    }

    fn circle_meets_rect(center: Point, radius: f32, rect: Rect) -> bool {
        let nearest = Point::new(
            center.x.clamp(rect.left, rect.right),
            center.y.clamp(rect.top, rect.bottom),
        );
        nearest.distance(center) < radius
    }

    #[test]
    fn layout_fits_every_phone_without_overlaps() {
        for (screen, insets) in PHONES {
            let layout = Layout::new(screen, insets);
            let mut drawn: Vec<(String, Rect)> = layout
                .buttons
                .iter()
                .map(|b| (format!("{:?}", b.button), b.shape.bounds(b.center)))
                .collect();
            drawn.extend(layout.dpad.bars().map(|bar| ("D-pad".to_string(), bar)));

            for (i, (name, rect)) in drawn.iter().enumerate() {
                assert!(
                    layout.area.contains_rect(*rect),
                    "{name} leaves the usable area on {screen:?}"
                );
                for (other, other_rect) in &drawn[i + 1..] {
                    if name != other {
                        assert!(
                            !rect.overlaps(*other_rect),
                            "{name} overlaps {other} on {screen:?}"
                        );
                    }
                }
            }

            // The stick at rest fits, and wherever its center moves it
            // covers no other control.
            let stick = layout.stick;
            let rest = Shape::Circle {
                radius: stick.reach(),
            }
            .bounds(stick.center);
            assert!(
                layout.area.contains_rect(rest),
                "the stick leaves the usable area on {screen:?}"
            );
            let footprint = stick.reach() + stick.recenter_radius();
            for (name, rect) in &drawn {
                assert!(
                    !circle_meets_rect(stick.center, footprint, *rect),
                    "the stick can cover {name} on {screen:?}"
                );
            }
            assert!(
                stick.center.x < layout.area.center_x(),
                "the stick rests on its side"
            );
        }
    }

    #[test]
    fn frequent_buttons_are_at_least_44_points() {
        let layout = remote().layout().clone();
        for b in &layout.buttons {
            let bounds = b.shape.bounds(b.center);
            assert!(
                bounds.right - bounds.left >= 44.0 && bounds.bottom - bounds.top >= 44.0,
                "{:?}",
                b.button
            );
        }
    }

    #[test]
    fn every_button_is_pressed_alone_at_its_center() {
        for (screen, insets) in PHONES {
            let mut remote = N64Remote::new(screen, insets);
            let buttons: Vec<_> = remote.layout().buttons.clone();
            for (id, layout) in (0..).zip(&buttons) {
                assert_eq!(remote.touch_down(id, layout.center), Some(Haptic::Press));
                assert_eq!(held(&remote), vec![layout.button]);
                assert_eq!(remote.touch_up(id), Some(Haptic::Release));
            }
            assert_eq!(remote.pad(), N64Pad::default());
        }
    }

    #[test]
    fn one_thumb_between_a_and_b_presses_both() {
        let mut remote = remote();
        let (a, b) = (
            center_of(&remote, N64Button::A),
            center_of(&remote, N64Button::B),
        );
        remote.touch_down(1, Point::new(a.x.midpoint(b.x), a.y.midpoint(b.y)));
        assert_eq!(held(&remote), [N64Button::A, N64Button::B]);
    }

    #[test]
    fn one_thumb_between_two_c_buttons_presses_one() {
        let mut remote = remote();
        let (up, right) = (
            center_of(&remote, N64Button::CUp),
            center_of(&remote, N64Button::CRight),
        );
        remote.touch_down(
            1,
            Point::new(up.x.midpoint(right.x), up.y.midpoint(right.y)),
        );
        assert_eq!(held(&remote).len(), 1);
    }

    #[test]
    fn sliding_from_b_to_a_rolls_through_both() {
        let mut remote = remote();
        let (a, b) = (
            center_of(&remote, N64Button::A),
            center_of(&remote, N64Button::B),
        );
        assert_eq!(remote.touch_down(1, b), Some(Haptic::Press));
        assert_eq!(
            remote.touch_move(1, Point::new(a.x.midpoint(b.x), a.y.midpoint(b.y))),
            Some(Haptic::Press)
        );
        assert_eq!(
            held(&remote),
            [N64Button::A, N64Button::B],
            "B stays held while rolling onto A"
        );
        assert_eq!(remote.touch_move(1, a), Some(Haptic::Release));
        assert_eq!(held(&remote), [N64Button::A]);
        let below = Point::new(a.x - 45.0, remote.layout().area.bottom - 2.0);
        assert_eq!(remote.touch_move(1, below), Some(Haptic::Release));
        assert_eq!(held(&remote), [], "between buttons nothing is held");
    }

    #[test]
    fn a_held_button_keeps_a_wider_margin() {
        let mut remote = remote();
        let a = center_of(&remote, N64Button::A);
        let edge = Point::new(a.x + 40.0 + 25.0, a.y);
        remote.touch_down(1, edge);
        assert_eq!(held(&remote), [], "25 pt off A is too far for a new touch");
        remote.touch_down(2, a);
        remote.touch_move(2, edge);
        assert_eq!(
            held(&remote),
            [N64Button::A],
            "but not for a finger already on A"
        );
    }

    #[test]
    fn two_thumbs_hold_two_buttons_and_lifting_one_keeps_the_other() {
        let mut remote = remote();
        remote.touch_down(1, center_of(&remote, N64Button::L));
        remote.touch_down(2, center_of(&remote, N64Button::A));
        assert_eq!(held(&remote), vec![N64Button::A, N64Button::L]);
        assert_eq!(remote.touch_up(1), Some(Haptic::Release));
        assert_eq!(held(&remote), vec![N64Button::A]);
    }

    #[test]
    fn stick_centers_where_the_thumb_lands_near_its_rest() {
        let mut remote = remote();
        let rest = remote.layout().stick.center;
        let landing = Point::new(rest.x + 20.0, rest.y - 15.0);
        assert_eq!(remote.touch_down(1, landing), None);
        assert_eq!(
            remote.pad(),
            N64Pad::default(),
            "landing off-center does not deflect"
        );
        assert_eq!(
            remote.stick().knob,
            rest,
            "the hidden recentering does not move the drawing"
        );
        remote.touch_move(1, Point::new(landing.x + 10.0, landing.y));
        assert_eq!(remote.stick().knob, Point::new(rest.x + 10.0, rest.y));
    }

    #[test]
    fn a_thumb_landing_far_from_the_stick_already_pushes_it() {
        let mut remote = remote();
        let layout = remote.layout().clone();
        let landing = Point::new(layout.area.left + 10.0, layout.area.bottom - 5.0);
        remote.touch_down(1, landing);
        let knob = remote.stick().knob;
        assert!(
            knob.x < layout.stick.center.x && knob.y > layout.stick.center.y,
            "the knob leans toward the thumb"
        );
        let pad = remote.pad();
        assert!(
            pad.stick_x < -60 && pad.stick_y < -10,
            "leans down-left at once: {pad:?}"
        );
    }

    #[test]
    fn the_whole_left_half_takes_the_stick() {
        let mut remote = remote();
        let area = remote.layout().area;
        remote.touch_down(
            1,
            Point::new(area.center_x() - 10.0, area.top + area.height() * 0.6),
        );
        assert!(remote.pad().stick_x > 0);
        remote.touch_up(1);
        remote.touch_down(
            2,
            Point::new(area.center_x() + 10.0, area.top + area.height() * 0.6),
        );
        assert_eq!(remote.pad(), N64Pad::default(), "the right half does not");
    }

    #[test]
    fn stick_reads_up_as_positive_and_saturates_at_the_gate() {
        let mut remote = remote();
        let travel = remote.layout().stick.travel;
        remote.touch_down(1, remote.layout().stick.center);
        let origin = remote.layout().stick.center;

        remote.touch_move(1, Point::new(origin.x, origin.y - travel * 0.5));
        let half = remote.pad();
        assert_eq!(half.stick_x, 0);
        assert_eq!(half.stick_y, 41, "half travel reads about half of 85");

        assert_eq!(
            remote.touch_move(1, Point::new(origin.x + travel * 3.0, origin.y)),
            Some(Haptic::Edge)
        );
        assert_eq!(
            (remote.pad().stick_x, remote.pad().stick_y),
            (N64Pad::STICK_CARDINAL, 0)
        );
        assert_eq!(
            remote.stick().knob,
            Point::new(origin.x + travel, origin.y),
            "the knob stops at the gate"
        );

        let diagonal = travel * 2.0;
        remote.touch_move(1, Point::new(origin.x - diagonal, origin.y + diagonal));
        let pad = remote.pad();
        assert_eq!(
            (pad.stick_x, pad.stick_y),
            (-N64Pad::STICK_DIAGONAL, -N64Pad::STICK_DIAGONAL),
            "the diagonal notch of a real gate, not 85/√2 = 60"
        );
        assert!(
            remote.stick().knob.distance(origin) > travel * 1.15,
            "the knob goes further into a diagonal corner"
        );
    }

    #[test]
    fn diagonal_rim_is_further_than_the_axis_rim() {
        let mut remote = remote();
        let travel = remote.layout().stick.travel;
        remote.touch_down(1, remote.layout().stick.center);
        let origin = remote.layout().stick.center;
        let step = travel * std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(
            remote.touch_move(1, Point::new(origin.x + step, origin.y - step)),
            None,
            "one travel along a diagonal is not yet the gate"
        );
        let pad = remote.pad();
        assert!(
            pad.stick_x == pad.stick_y && (58..=62).contains(&pad.stick_x),
            "{pad:?}"
        );
        assert_eq!(
            remote.touch_move(1, Point::new(origin.x + step * 1.2, origin.y - step * 1.2)),
            Some(Haptic::Edge)
        );
    }

    #[test]
    fn stick_dead_zone_only_filters_jitter() {
        let mut remote = remote();
        let travel = remote.layout().stick.travel;
        remote.touch_down(1, remote.layout().stick.center);
        let origin = remote.layout().stick.center;
        remote.touch_move(
            1,
            Point::new(origin.x + travel * STICK_DEAD_ZONE * 0.9, origin.y),
        );
        assert_eq!(remote.pad(), N64Pad::default());
        remote.touch_move(1, Point::new(origin.x + travel * 0.1, origin.y));
        assert!(
            (5..=7).contains(&remote.pad().stick_x),
            "no jump past the dead zone: {}",
            remote.pad().stick_x
        );
    }

    #[test]
    fn rim_haptic_fires_once_until_the_stick_comes_back() {
        let mut remote = remote();
        let travel = remote.layout().stick.travel;
        remote.touch_down(1, remote.layout().stick.center);
        let origin = remote.layout().stick.center;
        let at = |f: f32| Point::new(origin.x + travel * f, origin.y);
        assert_eq!(remote.touch_move(1, at(1.0)), Some(Haptic::Edge));
        assert_eq!(remote.touch_move(1, at(1.5)), None);
        assert_eq!(
            remote.touch_move(1, at(0.9)),
            None,
            "inside the hysteresis band"
        );
        assert_eq!(remote.touch_move(1, at(1.0)), None);
        remote.touch_move(1, at(0.5));
        assert_eq!(remote.touch_move(1, at(1.0)), Some(Haptic::Edge));
    }

    #[test]
    fn a_thumb_that_leaves_the_stick_keeps_steering_and_presses_nothing() {
        let mut remote = remote();
        remote.touch_down(1, remote.layout().stick.center);
        remote.touch_move(1, center_of(&remote, N64Button::A));
        assert_eq!(held(&remote), []);
        assert!(remote.pad().stick_x > 70);
    }

    #[test]
    fn a_second_thumb_on_the_stick_side_is_ignored() {
        let mut remote = remote();
        remote.touch_down(1, remote.layout().stick.center);
        let origin = remote.layout().stick.center;
        remote.touch_down(2, Point::new(origin.x + 10.0, origin.y));
        remote.touch_move(2, Point::new(origin.x + 200.0, origin.y));
        assert_eq!(remote.pad(), N64Pad::default());
        assert_eq!(remote.stick().knob, origin);
    }

    #[test]
    fn dpad_favors_straight_directions() {
        let mut remote = remote();
        let dpad = remote.layout().dpad;
        let r = dpad.size * 0.4;
        let cases = [
            (0.0, -1.0, vec![N64Button::DUp]),
            (1.0, -1.0, vec![N64Button::DUp, N64Button::DRight]),
            (1.0, -0.45, vec![N64Button::DRight]),
            (1.0, 0.0, vec![N64Button::DRight]),
            (1.0, 1.0, vec![N64Button::DDown, N64Button::DRight]),
            (0.0, 1.0, vec![N64Button::DDown]),
            (-0.45, 1.0, vec![N64Button::DDown]),
            (-1.0, 0.0, vec![N64Button::DLeft]),
        ];
        remote.touch_down(1, dpad.center);
        assert_eq!(held(&remote), [], "the center of the cross is neutral");
        for (dx, dy, expected) in cases {
            remote.touch_move(
                1,
                Point::new(dpad.center.x + dx * r, dpad.center.y + dy * r),
            );
            assert_eq!(held(&remote), expected, "direction ({dx}, {dy})");
        }
    }

    #[test]
    fn resize_lets_go_of_everything() {
        let mut remote = remote();
        remote.touch_down(1, center_of(&remote, N64Button::A));
        let (screen, insets) = PHONES[1];
        remote.resize(screen, insets);
        assert_eq!(remote.pad(), N64Pad::default());
        remote.touch_move(1, center_of(&remote, N64Button::A));
        assert_eq!(
            remote.pad(),
            N64Pad::default(),
            "a finger from before the resize is forgotten"
        );
    }

    #[test]
    fn touches_on_empty_space_do_nothing() {
        let mut remote = remote();
        let area = remote.layout().area;
        let empty = Point::new(area.center_x() + 5.0, area.top + area.height() * 0.45);
        assert_eq!(remote.touch_down(1, empty), None);
        remote.touch_move(1, center_of(&remote, N64Button::A));
        assert_eq!(remote.pad(), N64Pad::default());
    }
}
