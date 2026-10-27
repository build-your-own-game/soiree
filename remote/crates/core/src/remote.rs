//! A pad remote, laid out from a game's [`remote_protocol::Layout`]: the
//! shared buttons, D-pads and sticks, plus the tilt, pointer and canvas the
//! layout asks for. The app forwards touches with their time and motion
//! samples, draws what this says and plays the haptics returned.

use std::collections::BTreeMap;

use remote_protocol::{
    self as protocol, Action, DrawEvent, InputState, Kind, StickValue, to_i8, to_u16,
};

use crate::controls::{ButtonId, Layout, Pressed, TouchPad};
use crate::draw::Canvas;
use crate::pointer::Pointer;
use crate::wheel::Wheel;
use crate::{Haptic, Insets, Orientation, Point, Rect, Size};

/// One motion sample from the phone's sensors, in the device's axes (x to
/// the right of the portrait screen, y to its top, z out of it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Gravity, in g, fused by the system from accelerometer and gyroscope.
    pub gravity: [f32; 3],
    /// Rotation rate, radians per second, bias removed.
    pub rotation_rate: [f32; 3],
    /// Seconds, on the same clock as touches.
    pub time: f64,
    /// How the interface is turned.
    pub orientation: Orientation,
}

#[derive(Clone, Debug)]
pub struct Remote {
    spec: protocol::Layout,
    touch: TouchPad,
    wheel: Option<Wheel>,
    /// The pointer, and the fingers dragging it with their last position.
    pointer: Option<(Pointer, BTreeMap<u64, Point>)>,
    canvas: Option<Canvas>,
    actions: BTreeMap<ButtonId, Action>,
}

impl Remote {
    pub fn new(spec: protocol::Layout, screen: Size, insets: Insets) -> Self {
        let layout = Layout::resolve(&spec, screen, insets);
        let actions = spec
            .controls
            .iter()
            .filter_map(|c| {
                if let Kind::Action { action, .. } = c.kind {
                    Some((c.id, action))
                } else {
                    None
                }
            })
            .collect();
        Self {
            wheel: spec.tilt.then(Wheel::default),
            pointer: spec.pointer.then(|| (Pointer::default(), BTreeMap::new())),
            canvas: spec.canvas.then(|| Canvas::new(layout.area)),
            touch: TouchPad::new(layout),
            actions,
            spec,
        }
    }

    /// The layout as the game sent it: labels, colors, kinds.
    pub fn spec(&self) -> &protocol::Layout {
        &self.spec
    }

    /// The layout placed on this screen.
    pub fn layout(&self) -> &Layout {
        self.touch.layout()
    }

    pub fn resize(&mut self, screen: Size, insets: Insets) {
        let layout = Layout::resolve(&self.spec, screen, insets);
        if let Some(canvas) = &mut self.canvas {
            canvas.set_area(layout.area);
        }
        if let Some((_, fingers)) = &mut self.pointer {
            fingers.clear();
        }
        self.touch.set_layout(layout);
    }

    pub fn touch_down(&mut self, id: u64, point: Point, time: f64) -> Option<Haptic> {
        let before = self.touch.pressed();
        let haptic = self.touch.touch_down(id, point);
        let new = Pressed(self.touch.pressed().0 & !before.0);
        for action in new.ids().filter_map(|id| self.actions.get(&id).copied()) {
            match action {
                Action::Recenter => {
                    if let Some((pointer, _)) = &mut self.pointer {
                        pointer.recenter();
                    }
                }
                tool @ (Action::Color(_) | Action::Size(_) | Action::Undo | Action::Clear) => {
                    if let Some(canvas) = &mut self.canvas {
                        canvas.act(tool, time);
                    }
                }
            }
        }
        if !self.touch.holds(id) {
            if let Some(canvas) = &mut self.canvas {
                // Off the canvas, or a second finger: it does not draw, and
                // `touch_move` asks the canvas whose finger it is.
                let _: bool = canvas.begin(id, point, time);
            }
            if let Some((_, fingers)) = &mut self.pointer {
                fingers.insert(id, point);
            }
        }
        haptic
    }

    pub fn touch_move(&mut self, id: u64, point: Point, time: f64) -> Option<Haptic> {
        if let Some(canvas) = self.canvas.as_mut().filter(|c| c.owns(id)) {
            canvas.extend(id, point, time);
            return None;
        }
        if let Some((pointer, fingers)) = &mut self.pointer
            && let Some(last) = fingers.get_mut(&id)
        {
            pointer.drag(point.minus(*last));
            *last = point;
            return None;
        }
        self.touch.touch_move(id, point)
    }

    pub fn touch_up(&mut self, id: u64) -> Option<Haptic> {
        if let Some(canvas) = self.canvas.as_mut().filter(|c| c.owns(id)) {
            canvas.end(id);
            return None;
        }
        if let Some((_, fingers)) = &mut self.pointer
            && fingers.remove(&id).is_some()
        {
            return None;
        }
        self.touch.touch_up(id)
    }

    pub fn motion(&mut self, motion: Motion) {
        if let Some(wheel) = &mut self.wheel {
            wheel.update(motion.gravity, motion.orientation);
        }
        if let Some((pointer, _)) = &mut self.pointer {
            pointer.motion(
                motion.gravity,
                motion.rotation_rate,
                motion.time,
                motion.orientation,
            );
        }
    }

    /// The app lost its touches (app switcher, phone call): nothing may stay
    /// held, and a stroke keeps what was drawn.
    pub fn release_all(&mut self) {
        self.touch.release_all();
        if let Some(canvas) = &mut self.canvas {
            canvas.finish();
        }
        if let Some((_, fingers)) = &mut self.pointer {
            fingers.clear();
        }
    }

    pub fn pressed(&self) -> Pressed {
        self.touch.pressed()
    }

    /// Tools shown as selected: the drawing color and width, and Clear while
    /// it waits for its second tap.
    pub fn selected(&self, time: f64) -> Vec<ButtonId> {
        let Some(canvas) = &self.canvas else {
            return Vec::new();
        };
        self.actions
            .iter()
            .filter(|(_, action)| canvas.is_selected(**action, time))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Where to draw each stick's knob.
    pub fn knobs(&self) -> Vec<(ButtonId, Point)> {
        self.touch.knobs()
    }

    pub fn wheel(&self) -> Option<&Wheel> {
        self.wheel.as_ref()
    }

    pub fn pointer(&self) -> Option<&Pointer> {
        self.pointer.as_ref().map(|(pointer, _)| pointer)
    }

    pub fn canvas(&self) -> Option<&Canvas> {
        self.canvas.as_ref()
    }

    pub fn canvas_rect(&self) -> Option<Rect> {
        self.canvas.as_ref().map(Canvas::rect)
    }

    /// The state a game reads, numbered `seq`.
    pub fn state(&self, seq: u32) -> InputState {
        InputState {
            seq,
            pressed: self.touch.pressed().0,
            sticks: self
                .touch
                .sticks()
                .into_iter()
                .map(|(id, value)| StickValue {
                    id,
                    x: to_i8(value.x),
                    y: to_i8(value.y),
                })
                .collect(),
            tilt: self.wheel.map_or(0, |w| to_i8(w.steering)),
            pointer: self
                .pointer()
                .map(|p| [to_u16(p.cursor.x), to_u16(p.cursor.y)]),
        }
    }

    /// Drawing steps the game has not received yet.
    pub fn drain_drawing(&mut self) -> Vec<DrawEvent> {
        self.canvas.as_mut().map(Canvas::drain).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use remote_protocol::presets;

    use super::*;
    use crate::tests::PHONES;

    fn remote(spec: protocol::Layout) -> Remote {
        let (screen, insets) = PHONES[0];
        Remote::new(spec, screen, insets)
    }

    fn center_of(remote: &Remote, id: ButtonId) -> Point {
        remote
            .layout()
            .buttons
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.center)
            .unwrap_or_default()
    }

    #[test]
    fn every_preset_lays_out_and_answers_touches() {
        for (name, spec) in presets::all() {
            let mut remote = remote(spec);
            let first = remote.layout().buttons[0];
            assert!(remote.touch_down(1, first.center, 0.0).is_some(), "{name}");
            assert!(!remote.pressed().is_empty(), "{name}");
            remote.release_all();
            assert!(remote.pressed().is_empty());
        }
    }

    #[test]
    fn a_finger_off_the_pointer_buttons_drags_the_cursor_and_center_brings_it_back() {
        let mut remote = remote(presets::pointer());
        let area = remote.layout().area;
        let start = Point::new(area.center_x(), area.center_y());
        remote.touch_down(1, start, 0.0);
        remote.touch_move(1, Point::new(start.x + 60.0, start.y), 0.1);
        let (x, _) = remote.state(0).pointer().unwrap_or_default();
        assert!((x - 0.6).abs() < 1e-3, "{x}");
        remote.touch_up(1);
        remote.touch_down(2, center_of(&remote, presets::pointer::CENTER), 0.2);
        assert_eq!(remote.state(1).pointer, Some([32768, 32768]));
    }

    #[test]
    fn drawing_on_the_canvas_with_the_tools_beside_it() {
        let mut remote = remote(presets::draw());
        let rect = remote.canvas_rect().unwrap_or_default();
        let p = Point::new(rect.center_x(), rect.center_y());
        remote.touch_down(9, center_of(&remote, 2), 0.0);
        remote.touch_up(9);
        assert_eq!(
            remote.selected(0.0),
            [2, presets::draw::FIRST_SIZE + 1],
            "red, middle width"
        );
        assert_eq!(remote.touch_down(1, p, 0.1), None);
        remote.touch_move(1, Point::new(p.x + 30.0, p.y), 0.15);
        remote.touch_up(1);
        assert_eq!(remote.canvas().map(|c| c.strokes().len()), Some(1));
        remote.touch_down(2, center_of(&remote, presets::draw::UNDO), 1.0);
        assert_eq!(remote.canvas().map(|c| c.strokes().len()), Some(0));
        let events = remote.drain_drawing();
        assert!(matches!(
            events.first(),
            Some(DrawEvent::Begin {
                color: 0x00E6_3946,
                ..
            })
        ));
        assert_eq!(events.last(), Some(&DrawEvent::Undo));
    }

    #[test]
    fn the_wheel_steers_from_motion() {
        let mut remote = remote(presets::wheel());
        let tilt = 30.0_f32.to_radians();
        remote.motion(Motion {
            gravity: [libm::cosf(tilt), libm::sinf(tilt), 0.0],
            rotation_rate: [0.0; 3],
            time: 0.0,
            orientation: Orientation::LandscapeLeft,
        });
        assert_eq!(remote.state(0).tilt, 127);
    }

    #[test]
    fn a_new_size_keeps_the_drawing() {
        let mut remote = remote(presets::draw());
        let rect = remote.canvas_rect().unwrap_or_default();
        remote.touch_down(1, Point::new(rect.center_x(), rect.center_y()), 0.0);
        remote.touch_up(1);
        let (screen, insets) = PHONES[1];
        remote.resize(screen, insets);
        assert_eq!(remote.canvas().map(|c| c.strokes().len()), Some(1));
    }
}
