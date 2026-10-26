//! The touch engine every pad-like remote shares: buttons, D-pads and
//! sticks, laid out from a game's [`remote_protocol::Layout`]. The game says
//! what and where; the rules below, each with its source in `LEARNINGS.md`,
//! give the feel:
//! - a finger owns what it first touched until it lifts, so a thumb that
//!   drifts off a stick keeps steering and never presses a button;
//! - a thumb landing on a stick's half of the screen takes it. Near the
//!   stick's rest position, the landing point becomes its center; further out,
//!   the center stops on a small circle, so the stick already leans toward
//!   the thumb. The gate stays drawn in place: only the knob moves;
//! - sliding from one button to another presses the new one. A finger
//!   between two chord buttons presses both; elsewhere the nearest button
//!   wins, and a held button keeps a wider margin so it does not flicker on
//!   its edge;
//! - a D-pad favors straight directions and keeps its direction wherever the
//!   finger goes;
//! - sizes are in points: a thumb has the same size on every iPhone.
//!
//! A layout comes from a game, over the network: anything malformed (an id
//! past 31, a negative or infinite size) is left out rather than trusted.

use std::collections::BTreeMap;

use remote_protocol::{self as protocol, Anchor, Kind, MAX_ID};

use crate::{Haptic, Insets, Point, Rect, Size};

/// Reach added around each button and D-pad, in points.
pub const SLOP: f32 = 15.0;
/// Stick deflection ignored around the center, as a fraction of the travel:
/// only a filter for finger jitter, since games add their own dead zone.
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
/// The N64 gate: a diagonal notch reads 70 on both axes where an axis reads 85.
const OCTAGON_DIAGONAL: f32 = 70.0 / 85.0;
/// Corner radius of a button that fills the screen.
const FILL_CORNER: f32 = 40.0;

/// A control's bit in [`Pressed`]: 0 to [`MAX_ID`].
pub type ButtonId = u8;

/// Pressed buttons and D-pad directions, one bit per [`ButtonId`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pressed(pub u32);

impl Pressed {
    pub fn contains(self, id: ButtonId) -> bool {
        self.0
            .checked_shr(u32::from(id))
            .is_some_and(|bits| bits & 1 == 1)
    }

    /// An id past [`MAX_ID`] has no bit and is ignored.
    pub fn set(&mut self, id: ButtonId, pressed: bool) {
        let Some(bit) = 1u32.checked_shl(u32::from(id)) else {
            return;
        };
        if pressed {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn ids(self) -> impl Iterator<Item = ButtonId> {
        (0..=MAX_ID).filter(move |id| self.contains(*id))
    }
}

/// A button's outline in points. A full-screen button is resolved to a
/// rounded rectangle.
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
    RoundedRect {
        width: f32,
        height: f32,
        corner: f32,
    },
}

impl Shape {
    pub fn bounds(self, center: Point) -> Rect {
        let (half_width, half_height) = match self {
            Self::Circle { radius } => (radius, radius),
            Self::Capsule { width, height } | Self::RoundedRect { width, height, .. } => {
                (width / 2.0, height / 2.0)
            }
        };
        Rect {
            left: center.x - half_width,
            top: center.y - half_height,
            right: center.x + half_width,
            bottom: center.y + half_height,
        }
    }

    /// Distance from `point` to the shape's edge, negative inside.
    pub fn signed_distance(self, center: Point, point: Point) -> f32 {
        let offset = point.minus(center);
        match self {
            Self::Circle { radius } => offset.length() - radius,
            Self::Capsule { width, height } => {
                let radius = height / 2.0;
                let along = (offset.x.abs() - (width - height) / 2.0).max(0.0);
                Point::new(along, offset.y).length() - radius
            }
            Self::RoundedRect {
                width,
                height,
                corner,
            } => {
                let qx = offset.x.abs() - (width / 2.0 - corner);
                let qy = offset.y.abs() - (height / 2.0 - corner);
                Point::new(qx.max(0.0), qy.max(0.0)).length() + qx.max(qy).min(0.0) - corner
            }
        }
    }

    /// The shape in points, or `None` for a size that is negative or not a
    /// number. A full-screen shape fills `area`.
    fn resolve(shape: protocol::Shape, area: Rect) -> Option<Self> {
        let resolved = match shape {
            protocol::Shape::Circle { radius } => Self::Circle { radius },
            protocol::Shape::Capsule { width, height } => Self::Capsule {
                width,
                height: height.min(width),
            },
            protocol::Shape::RoundedRect {
                width,
                height,
                corner,
            } => Self::RoundedRect {
                width,
                height,
                corner: corner.min(width / 2.0).min(height / 2.0),
            },
            protocol::Shape::Fill { margin } => Self::RoundedRect {
                width: area.width() - 2.0 * margin,
                height: area.height() - 2.0 * margin,
                corner: FILL_CORNER,
            },
        };
        let sizes = match resolved {
            Self::Circle { radius } => [radius, radius, 0.0],
            Self::Capsule { width, height } => [width, height, 0.0],
            Self::RoundedRect {
                width,
                height,
                corner,
            } => [width, height, corner],
        };
        sizes
            .iter()
            .all(|s| s.is_finite() && *s >= 0.0)
            .then_some(resolved)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonLayout {
    pub id: ButtonId,
    pub center: Point,
    pub shape: Shape,
    /// Reach beyond the drawing for a new touch: [`SLOP`] for a play button,
    /// 0 for a minor one (Start, Select). A finger already holding it gets
    /// twice as much.
    pub slop: f32,
    pub chord: bool,
}

/// Where the stick stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gate {
    Round,
    /// The N64's octagon. `diagonal` is how far each axis reads in a
    /// diagonal notch, relative to full reach along an axis.
    Octagon {
        diagonal: f32,
    },
}

impl Gate {
    /// Distance from the center to the gate along a unit direction, in
    /// travel units (1 along an axis).
    fn reach(self, x: f32, y: f32) -> f32 {
        match self {
            Self::Round => 1.0,
            Self::Octagon { diagonal } => {
                let (major, minor) = (x.abs().max(y.abs()), x.abs().min(y.abs()));
                // The gate edge from (1, 0) to (diagonal, diagonal), whose
                // outward normal is (diagonal, 1 - diagonal).
                diagonal / (diagonal * major + (1.0 - diagonal) * minor)
            }
        }
    }

    /// How far the gate reaches along a diagonal, relative to an axis.
    pub fn diagonal_reach(self) -> f32 {
        self.reach(
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        )
    }

    /// `offset` in travel units, stopped at the gate.
    fn clamp(self, offset: Point) -> Point {
        let length = offset.length();
        if length == 0.0 {
            return offset;
        }
        let reach = self.reach(offset.x / length, offset.y / length);
        if length <= reach {
            offset
        } else {
            offset.scaled(reach / length)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickLayout {
    pub id: ButtonId,
    /// Where the stick rests, and where it is drawn.
    pub center: Point,
    /// How far the knob's center travels to full deflection along an axis.
    pub travel: f32,
    pub knob_radius: f32,
    pub gate: Gate,
    /// A thumb landing here takes the stick: the half of the screen it is in.
    pub zone: Rect,
}

impl StickLayout {
    /// Room the drawn stick needs around its center.
    pub fn reach(self) -> f32 {
        self.travel * self.gate.diagonal_reach() + self.knob_radius
    }

    /// How far the hidden center can move from the rest position.
    pub fn recenter_radius(self) -> f32 {
        RECENTER_ZONE * self.travel
    }

    /// Where a stick taken at `point` is centered: there if the thumb landed
    /// close to the rest position, else on the recentering circle toward it.
    fn origin_for(self, point: Point) -> Point {
        let offset = point.minus(self.center);
        let (length, zone) = (offset.length(), self.recenter_radius());
        if length <= zone {
            point
        } else {
            self.center.plus(offset.scaled(zone / length))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DPadLayout {
    pub center: Point,
    /// Width and height of the cross; each arm is a third of it wide.
    pub size: f32,
    /// The directions' ids: up, down, left, right.
    pub ids: [ButtonId; 4],
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

    fn contains(self, point: Point) -> bool {
        let reach = self.size / 2.0 + SLOP;
        let offset = point.minus(self.center);
        offset.x.abs() <= reach && offset.y.abs() <= reach
    }

    /// The directions a finger at `point` presses.
    fn directions(self, point: Point) -> Pressed {
        let [up, down, left, right] = self.ids;
        let offset = point.minus(self.center);
        let mut pressed = Pressed::default();
        if offset.length() < DPAD_DEAD_ZONE * self.size {
            return pressed;
        }
        if offset.x.abs() > TAN_25 * offset.y.abs() {
            pressed.set(if offset.x > 0.0 { right } else { left }, true);
        }
        if offset.y.abs() > TAN_25 * offset.x.abs() {
            pressed.set(if offset.y > 0.0 { down } else { up }, true);
        }
        pressed
    }
}

/// A layout resolved for one screen, in points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// The rectangle the controls fit in, outside the system's areas.
    pub area: Rect,
    pub buttons: Vec<ButtonLayout>,
    pub dpads: Vec<DPadLayout>,
    pub sticks: Vec<StickLayout>,
}

impl Layout {
    /// Places a game's layout on a screen.
    pub fn resolve(spec: &protocol::Layout, screen: Size, insets: Insets) -> Self {
        let area = Rect::usable(screen, insets);
        let mut layout = Self {
            area,
            ..Self::default()
        };
        for control in &spec.controls {
            let center =
                anchor(area, control.at.anchor).plus(Point::new(control.at.x, control.at.y));
            if !(center.x.is_finite() && center.y.is_finite()) || control.id > MAX_ID {
                continue;
            }
            match control.kind {
                Kind::Button {
                    shape,
                    chord,
                    minor,
                } => {
                    let fills = matches!(shape, protocol::Shape::Fill { .. });
                    // A full-screen button answers anywhere on the screen.
                    let slop = if minor {
                        0.0
                    } else if fills {
                        area.width().max(area.height())
                    } else {
                        SLOP
                    };
                    if let Some(shape) = Shape::resolve(shape, area) {
                        layout.buttons.push(ButtonLayout {
                            id: control.id,
                            center,
                            shape,
                            slop,
                            chord,
                        });
                    }
                }
                Kind::Action { shape, .. } => {
                    if let Some(shape) = Shape::resolve(shape, area) {
                        layout.buttons.push(ButtonLayout {
                            id: control.id,
                            center,
                            shape,
                            slop: SLOP,
                            chord: false,
                        });
                    }
                }
                Kind::DPad { size } => {
                    let ids = [control.id, control.id + 1, control.id + 2, control.id + 3];
                    if size.is_finite() && size > 0.0 && ids.iter().all(|id| *id <= MAX_ID) {
                        layout.dpads.push(DPadLayout { center, size, ids });
                    }
                }
                Kind::Stick {
                    travel,
                    knob_radius,
                    gate,
                } => {
                    if [travel, knob_radius]
                        .iter()
                        .all(|v| v.is_finite() && *v > 0.0)
                    {
                        let gate = match gate {
                            protocol::Gate::Round => Gate::Round,
                            protocol::Gate::Octagon => Gate::Octagon {
                                diagonal: OCTAGON_DIAGONAL,
                            },
                        };
                        let zone = if center.x < area.center_x() {
                            Rect {
                                right: area.center_x(),
                                ..area
                            }
                        } else {
                            Rect {
                                left: area.center_x(),
                                ..area
                            }
                        };
                        layout.sticks.push(StickLayout {
                            id: control.id,
                            center,
                            travel,
                            knob_radius,
                            gate,
                            zone,
                        });
                    }
                }
            }
        }
        layout
    }

    /// The buttons under `point` for a finger already holding `held`.
    fn buttons_at(&self, point: Point, held: Pressed) -> Pressed {
        let mut within: Vec<(f32, &ButtonLayout)> = Vec::new();
        for b in &self.buttons {
            let reach = if held.contains(b.id) {
                2.0 * b.slop
            } else {
                b.slop
            };
            let distance = b.shape.signed_distance(b.center, point);
            if distance <= reach {
                within.push((distance, b));
            }
        }
        let mut pressed = Pressed::default();
        let Some(&(_, nearest)) = within.iter().min_by(|a, b| a.0.total_cmp(&b.0)) else {
            return pressed;
        };
        if nearest.chord {
            for (_, b) in within.iter().filter(|(_, b)| b.chord) {
                pressed.set(b.id, true);
            }
        } else {
            pressed.set(nearest.id, true);
        }
        pressed
    }

    /// Every rectangle something is drawn in, named by what it is, for
    /// layout tests: buttons, then the D-pads' bars.
    pub fn drawn(&self) -> Vec<(String, Rect)> {
        let mut drawn: Vec<(String, Rect)> = self
            .buttons
            .iter()
            .map(|b| (format!("button {}", b.id), b.shape.bounds(b.center)))
            .collect();
        for dpad in &self.dpads {
            let name = format!("D-pad {}", dpad.ids[0]);
            drawn.extend(dpad.bars().map(|bar| (name.clone(), bar)));
        }
        drawn
    }
}

/// The point of `area` an anchor names.
fn anchor(area: Rect, anchor: Anchor) -> Point {
    let (x, y) = match anchor {
        Anchor::TopLeft => (area.left, area.top),
        Anchor::Top => (area.center_x(), area.top),
        Anchor::TopRight => (area.right, area.top),
        Anchor::Left => (area.left, area.center_y()),
        Anchor::Center => (area.center_x(), area.center_y()),
        Anchor::Right => (area.right, area.center_y()),
        Anchor::BottomLeft => (area.left, area.bottom),
        Anchor::Bottom => (area.center_x(), area.bottom),
        Anchor::BottomRight => (area.right, area.bottom),
    };
    Point::new(x, y)
}

/// What a finger is holding, from touch-down to lift.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Capture {
    /// `stick` indexes [`Layout::sticks`].
    Stick {
        stick: usize,
        origin: Point,
        position: Point,
    },
    DPad {
        dpad: usize,
        position: Point,
    },
    /// A finger on the buttons; none while it slides between them.
    Buttons {
        held: Pressed,
    },
}

#[derive(Clone, Debug, Default)]
pub struct TouchPad {
    layout: Layout,
    touches: BTreeMap<u64, Capture>,
    /// One rim haptic per stick until it comes back, by stick index.
    edge_armed: BTreeMap<usize, bool>,
}

impl TouchPad {
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            touches: BTreeMap::new(),
            edge_armed: BTreeMap::new(),
        }
    }

    /// A new layout (another remote, rotation) lets go of everything, like a
    /// lifted hand.
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.release_all();
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn touch_down(&mut self, id: u64, point: Point) -> Option<Haptic> {
        let before = self.pressed();
        if let Some(capture) = self.capture_at(point) {
            self.touches.insert(id, capture);
        }
        self.feedback(before)
    }

    pub fn touch_move(&mut self, id: u64, point: Point) -> Option<Haptic> {
        let before = self.pressed();
        let capture = self.touches.get(&id).copied()?;
        let moved = match capture {
            Capture::Stick { stick, origin, .. } => Capture::Stick {
                stick,
                origin,
                position: point,
            },
            Capture::DPad { dpad, .. } => Capture::DPad {
                dpad,
                position: point,
            },
            Capture::Buttons { held } => Capture::Buttons {
                held: self.layout.buttons_at(point, held),
            },
        };
        self.touches.insert(id, moved);
        self.feedback(before)
    }

    pub fn touch_up(&mut self, id: u64) -> Option<Haptic> {
        let before = self.pressed();
        self.touches.remove(&id);
        self.feedback(before)
    }

    /// Whether this finger holds a control.
    pub fn holds(&self, touch: u64) -> bool {
        self.touches.contains_key(&touch)
    }

    /// The app lost its touches (app switcher, phone call): nothing may stay
    /// held.
    pub fn release_all(&mut self) {
        self.touches.clear();
        self.edge_armed.clear();
    }

    /// Buttons and D-pad directions held by every finger at once.
    pub fn pressed(&self) -> Pressed {
        let mut pressed = Pressed::default();
        for capture in self.touches.values() {
            match *capture {
                Capture::DPad { dpad, position } => {
                    if let Some(dpad) = self.layout.dpads.get(dpad) {
                        pressed.0 |= dpad.directions(position).0;
                    }
                }
                Capture::Buttons { held } => pressed.0 |= held.0,
                Capture::Stick { .. } => {}
            }
        }
        pressed
    }

    /// Every stick's value, Y up: 1 is full deflection along an axis. Past
    /// the jitter filter it grows from 0, with no jump.
    pub fn sticks(&self) -> Vec<(ButtonId, Point)> {
        (0..self.layout.sticks.len())
            .filter_map(|index| self.stick_value(index))
            .collect()
    }

    /// Where to draw each stick's knob. Gates stay at rest: the hidden
    /// recentering under the thumb must not look like the stick jumped.
    pub fn knobs(&self) -> Vec<(ButtonId, Point)> {
        self.layout
            .sticks
            .iter()
            .enumerate()
            .map(|(index, stick)| {
                let knob = self
                    .stick_capture(index)
                    .map_or(stick.center, |(origin, position)| {
                        let offset = position.minus(origin);
                        stick.center.plus(
                            stick
                                .gate
                                .clamp(offset.scaled(1.0 / stick.travel))
                                .scaled(stick.travel),
                        )
                    });
                (stick.id, knob)
            })
            .collect()
    }

    fn stick_value(&self, index: usize) -> Option<(ButtonId, Point)> {
        let stick = self.layout.sticks.get(index)?;
        let Some((origin, position)) = self.stick_capture(index) else {
            return Some((stick.id, Point::default()));
        };
        let offset = position.minus(origin);
        let knob = stick.gate.clamp(Point::new(
            offset.x / stick.travel,
            -offset.y / stick.travel,
        ));
        let length = knob.length();
        if length <= STICK_DEAD_ZONE {
            return Some((stick.id, Point::default()));
        }
        Some((
            stick.id,
            knob.scaled((length - STICK_DEAD_ZONE) / (1.0 - STICK_DEAD_ZONE) / length),
        ))
    }

    fn capture_at(&self, point: Point) -> Option<Capture> {
        let held = self.layout.buttons_at(point, Pressed::default());
        if !held.is_empty() {
            return Some(Capture::Buttons { held });
        }
        if let Some(dpad) = self.layout.dpads.iter().position(|d| d.contains(point)) {
            return Some(Capture::DPad {
                dpad,
                position: point,
            });
        }
        let stick = self
            .layout
            .sticks
            .iter()
            .enumerate()
            .find(|(index, s)| s.zone.contains(point) && self.stick_capture(*index).is_none());
        stick.map(|(index, s)| Capture::Stick {
            stick: index,
            origin: s.origin_for(point),
            position: point,
        })
    }

    fn stick_capture(&self, index: usize) -> Option<(Point, Point)> {
        self.touches.values().find_map(|capture| match *capture {
            Capture::Stick {
                stick,
                origin,
                position,
            } if stick == index => Some((origin, position)),
            _ => None,
        })
    }

    /// Stick deflection from 0 (center) to 1 (gate) and beyond.
    fn deflection(&self, index: usize) -> f32 {
        let (Some(stick), Some((origin, position))) =
            (self.layout.sticks.get(index), self.stick_capture(index))
        else {
            return 0.0;
        };
        let offset = position.minus(origin);
        let length = offset.length();
        if length == 0.0 {
            return 0.0;
        }
        length / stick.travel / stick.gate.reach(offset.x / length, offset.y / length)
    }

    /// Whether a stick just reached its rim.
    fn update_edges(&mut self) -> bool {
        let mut edge = false;
        for index in 0..self.layout.sticks.len() {
            let deflection = self.deflection(index);
            let armed = self.edge_armed.entry(index).or_insert(true);
            if *armed && deflection >= EDGE_ON {
                *armed = false;
                edge = true;
            } else if deflection < EDGE_OFF {
                *armed = true;
            }
        }
        edge
    }

    fn feedback(&mut self, before: Pressed) -> Option<Haptic> {
        let edge = self.update_edges();
        let after = self.pressed().0;
        if after & !before.0 != 0 {
            Some(Haptic::Press)
        } else if before.0 & !after != 0 {
            Some(Haptic::Release)
        } else if edge {
            Some(Haptic::Edge)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_distance_is_zero_on_its_edge() {
        let shape = Shape::RoundedRect {
            width: 100.0,
            height: 40.0,
            corner: 10.0,
        };
        let center = Point::new(0.0, 0.0);
        assert!(shape.signed_distance(center, Point::new(50.0, 0.0)).abs() < 1e-5);
        assert!(shape.signed_distance(center, Point::new(0.0, -20.0)).abs() < 1e-5);
        assert!((shape.signed_distance(center, Point::new(0.0, 0.0)) + 20.0).abs() < 1e-5);
        assert!((shape.signed_distance(center, Point::new(60.0, 0.0)) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn round_gate_reaches_one_in_every_direction() {
        let gate = Gate::Round;
        assert_eq!(gate.diagonal_reach(), 1.0);
        let clamped = gate.clamp(Point::new(3.0, 4.0));
        assert!((clamped.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn pressed_lists_its_ids_and_ignores_ids_past_31() {
        let mut pressed = Pressed::default();
        pressed.set(3, true);
        pressed.set(31, true);
        pressed.set(32, true);
        pressed.set(200, true);
        assert_eq!(pressed.ids().collect::<Vec<_>>(), [3, 31]);
        pressed.set(3, false);
        assert!(pressed.contains(31) && !pressed.contains(3) && !pressed.contains(200));
    }

    #[test]
    fn a_malformed_layout_is_left_out_not_trusted() {
        use protocol::{Control, Label, Place};
        let control = |id, kind| Control {
            id,
            kind,
            at: Place::new(Anchor::Center, 0.0, 0.0),
            label: Label::None,
            color: 0,
        };
        let circle = |radius| protocol::Shape::Circle { radius };
        let spec = protocol::Layout {
            controls: vec![
                control(
                    40,
                    Kind::Button {
                        shape: circle(30.0),
                        chord: false,
                        minor: false,
                    },
                ),
                control(
                    1,
                    Kind::Button {
                        shape: circle(-5.0),
                        chord: false,
                        minor: false,
                    },
                ),
                control(
                    2,
                    Kind::Button {
                        shape: circle(f32::NAN),
                        chord: false,
                        minor: false,
                    },
                ),
                control(30, Kind::DPad { size: 100.0 }),
                control(
                    3,
                    Kind::Stick {
                        travel: f32::INFINITY,
                        knob_radius: 20.0,
                        gate: protocol::Gate::Round,
                    },
                ),
                control(
                    4,
                    Kind::Button {
                        shape: circle(30.0),
                        chord: false,
                        minor: false,
                    },
                ),
            ],
            ..protocol::Layout::default()
        };
        let screen = Size {
            width: 800.0,
            height: 400.0,
        };
        let layout = Layout::resolve(&spec, screen, Insets::default());
        assert_eq!(layout.buttons.iter().map(|b| b.id).collect::<Vec<_>>(), [4]);
        assert!(layout.dpads.is_empty() && layout.sticks.is_empty());
    }

    #[test]
    fn two_sticks_one_per_half() {
        use protocol::{Control, Label, Place};
        let stick = |id, anchor, x| Control {
            id,
            kind: Kind::Stick {
                travel: 50.0,
                knob_radius: 30.0,
                gate: protocol::Gate::Round,
            },
            at: Place::new(anchor, x, -100.0),
            label: Label::None,
            color: 0,
        };
        let spec = protocol::Layout {
            controls: vec![
                stick(0, Anchor::BottomLeft, 120.0),
                stick(1, Anchor::BottomRight, -120.0),
            ],
            ..protocol::Layout::default()
        };
        let mut pad = TouchPad::new(Layout::resolve(
            &spec,
            Size {
                width: 800.0,
                height: 400.0,
            },
            Insets::default(),
        ));
        pad.touch_down(1, Point::new(120.0, 300.0));
        pad.touch_move(1, Point::new(120.0, 200.0));
        pad.touch_down(2, Point::new(680.0, 300.0));
        pad.touch_move(2, Point::new(780.0, 300.0));
        assert_eq!(
            pad.sticks(),
            [(0, Point::new(0.0, 1.0)), (1, Point::new(1.0, 0.0))]
        );
    }
}
