//! The pointer of a layout with `pointer`: aim the phone at the TV, like a
//! Wii Remote, to move a cursor there. The phone is held sideways in both
//! hands, its back toward the TV.
//!
//! It is relative: the gyroscope's rotation rate moves the cursor, so nothing
//! depends on the magnetometer, which a TV disturbs. Turning left or right is
//! measured around true vertical ("player space"), so it feels the same
//! however the phone is tilted. The cursor stops at the TV's edges, which
//! recenters it the way a mouse does; a Recenter action puts it back in the
//! middle.
//! A finger dragged anywhere off the buttons moves it too, for fine aiming
//! and for phones (or the Simulator) without a gyroscope.

use crate::{Orientation, Point};

/// Turning the phone this many degrees moves the cursor across the TV
/// (Dolphin uses 25°, JoyShockMapper's default about 45°). Up and down use
/// the same degrees per point of TV.
pub const DEGREES_PER_WIDTH: f32 = 30.0;
const TV_ASPECT: f32 = 16.0 / 9.0;
/// Below this speed (degrees per second) motion is smoothed, above it never:
/// slow motion is mostly hand tremor, fast motion is meant ("soft tiered
/// smoothing", Jibb Smart's `GyroWiki`).
const SMOOTH_BELOW: f32 = 5.0;
const SMOOTH_TIME: f32 = 0.125;
/// A gap in motion samples longer than this is a pause, not a sample.
const MAX_STEP: f64 = 0.1;
/// Finger travel, in points, that moves the cursor across the TV.
const TRACKPAD_POINTS_PER_WIDTH: f32 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    /// Where the cursor is on the TV: 0 to 1 across, 0 to 1 down.
    pub cursor: Point,
    /// Motion data has arrived. Without it (the Simulator), only a finger
    /// moves the cursor.
    pub available: bool,
    /// Smoothed rotation, degrees per second: right, up.
    smoothed: (f32, f32),
    last_time: Option<f64>,
}

impl Default for Pointer {
    fn default() -> Self {
        Self {
            cursor: Point::new(0.5, 0.5),
            available: false,
            smoothed: (0.0, 0.0),
            last_time: None,
        }
    }
}

impl Pointer {
    /// `gravity` in g and `rotation_rate` in radians per second, in the
    /// device's axes (x to the right of the portrait screen, y to its top,
    /// z out of it), at `time` seconds.
    pub fn motion(
        &mut self,
        gravity: [f32; 3],
        rotation_rate: [f32; 3],
        time: f64,
        orientation: Orientation,
    ) {
        self.available = true;
        let dt = self.last_time.map_or(0.0, |last| time - last);
        self.last_time = Some(time);
        if !(0.0..=MAX_STEP).contains(&dt) {
            return;
        }
        #[expect(clippy::cast_possible_truncation, reason = "a step of at most 0.1 s")]
        let dt = dt as f32;

        // Turning right is a rotation around true vertical: the rate along
        // gravity, which points down.
        let length = libm::sqrtf(gravity.iter().map(|g| g * g).sum());
        if length < 0.5 {
            return;
        }
        let right = gravity
            .iter()
            .zip(rotation_rate)
            .map(|(g, w)| g / length * w)
            .sum::<f32>();
        // Aiming up rotates around the long axis, which points right in
        // landscape-left and left in landscape-right.
        let up = match orientation {
            Orientation::LandscapeLeft => rotation_rate[1],
            Orientation::LandscapeRight => -rotation_rate[1],
        };
        let raw = (right.to_degrees(), up.to_degrees());

        let speed = libm::hypotf(raw.0, raw.1);
        let direct = ((speed - SMOOTH_BELOW / 2.0) / (SMOOTH_BELOW / 2.0)).clamp(0.0, 1.0);
        let blend = 1.0 - libm::expf(-dt / SMOOTH_TIME);
        self.smoothed.0 += (raw.0 - self.smoothed.0) * blend;
        self.smoothed.1 += (raw.1 - self.smoothed.1) * blend;
        let rate = (
            direct * raw.0 + (1.0 - direct) * self.smoothed.0,
            direct * raw.1 + (1.0 - direct) * self.smoothed.1,
        );
        self.shift(
            rate.0 * dt / DEGREES_PER_WIDTH,
            -rate.1 * dt / DEGREES_PER_WIDTH * TV_ASPECT,
        );
    }

    /// A finger dragged by `delta` points.
    pub fn drag(&mut self, delta: Point) {
        self.shift(
            delta.x / TRACKPAD_POINTS_PER_WIDTH,
            delta.y / TRACKPAD_POINTS_PER_WIDTH * TV_ASPECT,
        );
    }

    pub fn recenter(&mut self) {
        self.cursor = Point::new(0.5, 0.5);
    }

    /// Past an edge the cursor stops, so turning back moves it at once.
    fn shift(&mut self, dx: f32, dy: f32) {
        self.cursor = Point::new(
            (self.cursor.x + dx).clamp(0.0, 1.0),
            (self.cursor.y + dy).clamp(0.0, 1.0),
        );
    }
}

#[cfg(test)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]
mod tests {
    use super::*;

    /// Upright in landscape-left: the device's x axis points down.
    const UPRIGHT: [f32; 3] = [1.0, 0.0, 0.0];

    /// Turns at `degrees_per_second` around `axis` for `seconds`, in 100 Hz
    /// samples.
    fn turn(pointer: &mut Pointer, axis: [f32; 3], degrees_per_second: f32, seconds: f64) {
        let rate = axis.map(|a| a * degrees_per_second.to_radians());
        let start = pointer.last_time.unwrap_or(0.0);
        let mut step = 0;
        while f64::from(step) * 0.01 <= seconds {
            pointer.motion(
                UPRIGHT,
                rate,
                start + f64::from(step) * 0.01,
                Orientation::LandscapeLeft,
            );
            step += 1;
        }
    }

    #[test]
    fn turning_15_degrees_right_crosses_half_the_tv() {
        let mut pointer = Pointer::default();
        // Turning right is a rotation around the down vector.
        turn(&mut pointer, UPRIGHT, 60.0, 0.25);
        assert!(
            (pointer.cursor.x - 1.0).abs() < 0.02,
            "{:?}",
            pointer.cursor
        );
        assert!((pointer.cursor.y - 0.5).abs() < 1e-4);
    }

    #[test]
    fn aiming_up_moves_the_cursor_up() {
        let mut pointer = Pointer::default();
        turn(&mut pointer, [0.0, 1.0, 0.0], 40.0, 0.1);
        assert!(pointer.cursor.y < 0.4, "{:?}", pointer.cursor);
        assert!((pointer.cursor.x - 0.5).abs() < 1e-4);
    }

    #[test]
    fn the_cursor_stops_at_the_edge_and_comes_back_at_once() {
        let mut pointer = Pointer::default();
        turn(&mut pointer, UPRIGHT, 120.0, 1.0);
        assert_eq!(pointer.cursor.x, 1.0);
        turn(&mut pointer, UPRIGHT, -60.0, 0.1);
        assert!(
            pointer.cursor.x < 0.9,
            "turning back moves it right away: {:?}",
            pointer.cursor
        );
    }

    #[test]
    fn hand_tremor_is_damped_but_a_real_move_is_not() {
        let mut slow = Pointer::default();
        turn(&mut slow, UPRIGHT, 2.0, 0.05);
        let tremor = slow.cursor.x - 0.5;
        assert!(
            tremor < 2.0 * 0.05 / 30.0 * 0.5,
            "a 2°/s wobble is mostly smoothed out: {tremor}"
        );

        let mut fast = Pointer::default();
        turn(&mut fast, UPRIGHT, 60.0, 0.05);
        let expected = 60.0 * 0.05 / 30.0;
        assert!(
            (fast.cursor.x - 0.5 - expected).abs() < 0.01,
            "{:?}",
            fast.cursor
        );
    }

    #[test]
    fn a_pause_in_samples_does_not_jump() {
        let mut pointer = Pointer::default();
        pointer.motion(UPRIGHT, [1.0, 0.0, 0.0], 0.0, Orientation::LandscapeLeft);
        pointer.motion(UPRIGHT, [1.0, 0.0, 0.0], 2.0, Orientation::LandscapeLeft);
        assert_eq!(pointer.cursor, Point::new(0.5, 0.5));
    }

    #[test]
    fn a_finger_moves_it_too_and_center_brings_it_back() {
        let mut pointer = Pointer::default();
        pointer.drag(Point::new(150.0, 0.0));
        assert!((pointer.cursor.x - 0.75).abs() < 1e-5);
        pointer.recenter();
        assert_eq!(pointer.cursor, Point::new(0.5, 0.5));
    }
}
