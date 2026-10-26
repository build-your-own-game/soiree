//! The drawing canvas of a layout with `canvas`, for Pictionary-like games:
//! 16:9 like the TV, between the screen's left and right tool columns.
//!
//! One finger draws; others are ignored while it does. Every touch sample
//! counts (the app passes the coalesced ones), samples closer than 1.5 pt are
//! dropped, and a short time-based average smooths the jitter, snapping to
//! the real last point when the finger lifts. A tap leaves a dot. Lines keep
//! one width: in a guessing game, readable beats stylish.
//!
//! Strokes live in canvas units, 4096 across and 2304 down, so a drawing
//! reads the same on every phone and the TV. A stroke may run past the edge.

use remote_protocol::{Action, CANVAS_HEIGHT, CANVAS_WIDTH, DrawEvent};

use crate::{Point, Rect};

/// Width of each tool column beside the canvas, in points.
const TOOL_COLUMN: f32 = 64.0;
/// Samples closer than this to the last kept one are dropped, in points.
const MIN_STEP: f32 = 1.5;
/// Time constant of the smoothing average, in seconds: about
/// perfect-freehand's "streamline 0.6" at 120 touch samples a second.
const SMOOTH_TIME: f64 = 0.012;
/// A second tap on Clear within this many seconds clears.
const CLEAR_CONFIRM: f64 = 2.0;
/// Before any tool is picked: black, 1 % of the canvas width.
const FIRST_COLOR: u32 = 0x001A_1A1A;
const FIRST_WIDTH: u16 = 41;

/// The largest 16:9 rectangle between the tool columns, centered.
pub fn canvas_rect(area: Rect) -> Rect {
    let room = Rect {
        left: area.left + TOOL_COLUMN,
        right: area.right - TOOL_COLUMN,
        ..area
    };
    let (canvas_width, canvas_height) = (f32::from(CANVAS_WIDTH), f32::from(CANVAS_HEIGHT));
    let width = room
        .width()
        .min(room.height() * canvas_width / canvas_height);
    let height = width * canvas_height / canvas_width;
    let (x, y) = (room.center_x(), room.center_y());
    Rect {
        left: x - width / 2.0,
        top: y - height / 2.0,
        right: x + width / 2.0,
        bottom: y + height / 2.0,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub id: u16,
    /// `0xRRGGBB`.
    pub color: u32,
    /// Canvas units.
    pub width: u16,
    /// Canvas units.
    pub points: Vec<Point>,
}

#[derive(Clone, Debug)]
struct Live {
    touch: u64,
    stroke: Stroke,
    /// Last sample kept, in points, and its time.
    raw: Point,
    time: f64,
    smoothed: Point,
}

#[derive(Clone, Debug)]
pub struct Canvas {
    /// Where the canvas is on screen, in points.
    rect: Rect,
    strokes: Vec<Stroke>,
    live: Option<Live>,
    color: u32,
    width: u16,
    clear_armed_until: Option<f64>,
    next_id: u16,
    /// Bumped whenever finished strokes change, so the app redraws them only
    /// then.
    revision: u32,
    outbox: Vec<DrawEvent>,
}

impl Canvas {
    pub fn new(area: Rect) -> Self {
        Self {
            rect: canvas_rect(area),
            strokes: Vec::new(),
            live: None,
            color: FIRST_COLOR,
            width: FIRST_WIDTH,
            clear_armed_until: None,
            next_id: 0,
            revision: 0,
            outbox: Vec::new(),
        }
    }

    /// The screen changed: the drawing stays, in canvas units.
    pub fn set_area(&mut self, area: Rect) {
        self.finish();
        self.rect = canvas_rect(area);
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    pub fn live(&self) -> Option<&Stroke> {
        self.live.as_ref().map(|live| &live.stroke)
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Takes the events the game has not received yet.
    pub fn drain(&mut self) -> Vec<DrawEvent> {
        std::mem::take(&mut self.outbox)
    }

    /// Whether this tool shows as selected: the current color and width,
    /// and Clear while it waits for its second tap.
    pub fn is_selected(&self, action: Action, time: f64) -> bool {
        match action {
            Action::Color(color) => color == self.color,
            Action::Size(width) => width == self.width,
            Action::Clear => self.clear_armed_until.is_some_and(|until| time <= until),
            Action::Undo | Action::Recenter => false,
        }
    }

    pub fn owns(&self, touch: u64) -> bool {
        self.live.as_ref().is_some_and(|live| live.touch == touch)
    }

    /// A finger landed on the canvas. Returns whether it now draws.
    pub fn begin(&mut self, touch: u64, point: Point, time: f64) -> bool {
        if self.live.is_some() || !self.rect.contains(point) {
            return false;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let first = to_canvas(self.rect, point);
        self.outbox.push(DrawEvent::Begin {
            stroke: id,
            color: self.color,
            width: self.width,
            x: wire(first.x),
            y: wire(first.y),
        });
        self.live = Some(Live {
            touch,
            stroke: Stroke {
                id,
                color: self.color,
                width: self.width,
                points: vec![first],
            },
            raw: point,
            time,
            smoothed: point,
        });
        true
    }

    /// One touch sample of the drawing finger.
    pub fn extend(&mut self, touch: u64, point: Point, time: f64) {
        let Some(live) = self.live.as_mut().filter(|live| live.touch == touch) else {
            return;
        };
        if point.distance(live.raw) < MIN_STEP {
            return;
        }
        let dt = (time - live.time).max(0.0);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a blend factor between 0 and 1"
        )]
        let blend = (1.0 - libm::exp(-dt / SMOOTH_TIME)) as f32;
        live.smoothed = live.smoothed.plus(point.minus(live.smoothed).scaled(blend));
        live.raw = point;
        live.time = time;
        let kept = to_canvas(self.rect, live.smoothed);
        live.stroke.points.push(kept);
        self.outbox.push(DrawEvent::Point {
            stroke: live.stroke.id,
            x: wire(kept.x),
            y: wire(kept.y),
        });
    }

    /// The drawing finger lifted: the stroke ends on its real last point.
    pub fn end(&mut self, touch: u64) {
        let Some(mut live) = self.live.take_if(|live| live.touch == touch) else {
            return;
        };
        if live.raw.distance(live.smoothed) >= MIN_STEP {
            let last = to_canvas(self.rect, live.raw);
            live.stroke.points.push(last);
            self.outbox.push(DrawEvent::Point {
                stroke: live.stroke.id,
                x: wire(last.x),
                y: wire(last.y),
            });
        }
        self.outbox.push(DrawEvent::End {
            stroke: live.stroke.id,
        });
        self.strokes.push(live.stroke);
        self.revision = self.revision.wrapping_add(1);
    }

    /// The drawing finger was taken away (app switcher, phone call): the
    /// stroke keeps what was drawn.
    pub fn finish(&mut self) {
        if let Some(touch) = self.live.as_ref().map(|live| live.touch) {
            self.end(touch);
        }
    }

    /// A tool button went down.
    pub fn act(&mut self, action: Action, time: f64) {
        match action {
            Action::Undo => {
                if self.strokes.pop().is_some() {
                    self.outbox.push(DrawEvent::Undo);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            Action::Clear => {
                if self.clear_armed_until.is_some_and(|until| time <= until) {
                    self.clear_armed_until = None;
                    self.strokes.clear();
                    self.outbox.push(DrawEvent::Clear);
                    self.revision = self.revision.wrapping_add(1);
                } else {
                    self.clear_armed_until = Some(time + CLEAR_CONFIRM);
                }
                return;
            }
            Action::Color(color) => self.color = color,
            Action::Size(width) => self.width = width,
            Action::Recenter => {}
        }
        self.clear_armed_until = None;
    }

    pub fn to_screen(&self, point: Point) -> Point {
        Point::new(
            self.rect.left + point.x / f32::from(CANVAS_WIDTH) * self.rect.width(),
            self.rect.top + point.y / f32::from(CANVAS_HEIGHT) * self.rect.height(),
        )
    }

    /// A line width in points on this screen.
    pub fn width_on_screen(&self, width: u16) -> f32 {
        f32::from(width) / f32::from(CANVAS_WIDTH) * self.rect.width()
    }
}

fn to_canvas(rect: Rect, point: Point) -> Point {
    Point::new(
        (point.x - rect.left) / rect.width() * f32::from(CANVAS_WIDTH),
        (point.y - rect.top) / rect.height() * f32::from(CANVAS_HEIGHT),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "rounded and clamped to i16 first"
)]
fn wire(value: f32) -> i16 {
    value
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::PHONES;

    fn canvas() -> Canvas {
        let (screen, insets) = PHONES[0];
        Canvas::new(Rect::usable(screen, insets))
    }

    fn middle(canvas: &Canvas) -> Point {
        Point::new(canvas.rect().center_x(), canvas.rect().center_y())
    }

    #[test]
    fn the_canvas_is_16_by_9_between_the_tool_columns() {
        for (screen, insets) in PHONES {
            let area = Rect::usable(screen, insets);
            let rect = canvas_rect(area);
            assert!((rect.width() / rect.height() - 16.0 / 9.0).abs() < 1e-3);
            assert!(
                rect.left >= area.left + TOOL_COLUMN - 0.01
                    && rect.right <= area.right - TOOL_COLUMN + 0.01
            );
        }
    }

    #[test]
    fn a_line_follows_the_finger_and_ends_on_its_last_point() {
        let mut canvas = canvas();
        let rect = canvas.rect();
        let start = Point::new(rect.left + 10.0, rect.center_y());
        assert!(canvas.begin(1, start, 0.0));
        for i in 1..=60u8 {
            canvas.extend(
                1,
                Point::new(start.x + f32::from(i) * 4.0, start.y),
                f64::from(i) / 120.0,
            );
        }
        canvas.end(1);
        let stroke = &canvas.strokes()[0];
        let last = stroke.points.last().copied().unwrap_or_default();
        assert!(
            (canvas.to_screen(last).x - (start.x + 240.0)).abs() < 0.01,
            "snaps to the real end"
        );
        assert!(
            stroke
                .points
                .iter()
                .all(|p| (canvas.to_screen(*p).y - start.y).abs() < 0.01)
        );
        assert_eq!(canvas.revision(), 1);
    }

    #[test]
    fn jitter_under_one_and_a_half_points_is_dropped() {
        let mut canvas = canvas();
        let p = middle(&canvas);
        canvas.begin(1, p, 0.0);
        canvas.extend(1, Point::new(p.x + 1.0, p.y), 0.01);
        canvas.extend(1, Point::new(p.x, p.y + 1.0), 0.02);
        canvas.end(1);
        assert_eq!(canvas.strokes()[0].points.len(), 1, "a tap is a dot");
    }

    #[test]
    fn a_second_finger_does_not_draw() {
        let mut canvas = canvas();
        let p = middle(&canvas);
        assert!(canvas.begin(1, p, 0.0));
        assert!(!canvas.begin(2, Point::new(p.x + 50.0, p.y), 0.0));
        canvas.extend(2, Point::new(p.x + 100.0, p.y), 0.1);
        assert_eq!(canvas.live().map(|s| s.points.len()), Some(1));
    }

    #[test]
    fn undo_removes_the_last_line_and_clear_needs_two_taps() {
        let mut canvas = canvas();
        let p = middle(&canvas);
        for touch in 0..3 {
            canvas.begin(touch, p, 0.0);
            canvas.end(touch);
        }
        canvas.act(Action::Undo, 1.0);
        assert_eq!(canvas.strokes().len(), 2);
        canvas.act(Action::Clear, 2.0);
        assert_eq!(canvas.strokes().len(), 2, "the first tap only arms");
        assert!(canvas.is_selected(Action::Clear, 2.5));
        canvas.act(Action::Clear, 3.0);
        assert_eq!(canvas.strokes(), []);
        canvas.act(Action::Clear, 10.0);
        canvas.act(Action::Clear, 13.0);
        assert!(
            canvas.is_selected(Action::Clear, 13.0),
            "too late: armed again, not cleared twice"
        );
    }

    #[test]
    fn color_and_width_apply_to_the_next_line() {
        let mut canvas = canvas();
        canvas.act(Action::Color(0x00FF_D60A), 0.0);
        canvas.act(Action::Size(82), 0.0);
        canvas.begin(1, middle(&canvas), 0.0);
        canvas.end(1);
        assert_eq!(
            (canvas.strokes()[0].color, canvas.strokes()[0].width),
            (0x00FF_D60A, 82)
        );
        assert!(canvas.is_selected(Action::Color(0x00FF_D60A), 0.0));
        assert!(!canvas.is_selected(Action::Size(41), 0.0));
    }

    #[test]
    fn events_tell_a_game_the_whole_story() {
        let mut canvas = canvas();
        let p = Point::new(canvas.rect().left, canvas.rect().top);
        canvas.begin(1, p, 0.0);
        canvas.extend(1, Point::new(p.x + 20.0, p.y), 0.1);
        canvas.end(1);
        canvas.act(Action::Undo, 1.0);
        let events = canvas.drain();
        assert_eq!(
            events.first(),
            Some(&DrawEvent::Begin {
                stroke: 0,
                color: FIRST_COLOR,
                width: FIRST_WIDTH,
                x: 0,
                y: 0
            })
        );
        assert!(matches!(events[1], DrawEvent::Point { stroke: 0, .. }));
        assert_eq!(
            events[events.len() - 2..],
            [DrawEvent::End { stroke: 0 }, DrawEvent::Undo]
        );
        assert_eq!(canvas.drain(), []);
    }
}
