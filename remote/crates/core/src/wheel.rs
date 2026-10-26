//! Steering by tilting the phone like a wheel, as with the Mario Kart Wii
//! wheel, for a layout with `tilt`.
//!
//! The angle comes from gravity as iOS fuses it from the accelerometer and
//! the gyroscope. It is the elevation of the phone's long axis, which reads
//! the same whether the phone is held upright or flat. Gravity is absolute:
//! no calibration, nothing drifts.

/// Tilting this far steers all the way: between SuperTuxKart (19.5°) and
/// PPSSPP (32°). The TV is the screen, so the phone may tilt far.
pub const FULL_LOCK_DEGREES: f32 = 25.0;
/// Tilt ignored around level, so a wheel held "straight" goes straight.
pub const DEAD_ZONE_DEGREES: f32 = 2.5;
/// Gravity in the screen's plane below this (in g): the phone lies flat, its
/// roll means nothing, and the last steering holds.
const MIN_IN_PLANE: f32 = 0.3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    /// The phone's tilt, degrees, positive to the right.
    pub angle: f32,
    /// -1 (full left) to 1 (full right).
    pub steering: f32,
    /// Motion data has arrived. Without it (the Simulator), steering stays 0.
    pub available: bool,
}

impl Wheel {
    /// `gravity` in g, in the device's axes (x to the right of the portrait
    /// screen, y to its top, z out of it). `landscape_left`: the interface is
    /// turned so the device's top points right.
    pub fn update(&mut self, gravity: [f32; 3], landscape_left: bool) {
        let [x, y, z] = gravity;
        self.available = true;
        if libm::hypotf(x, y) < MIN_IN_PLANE {
            return;
        }
        let sign = if landscape_left { 1.0 } else { -1.0 };
        self.angle = libm::atan2f(sign * y, libm::hypotf(x, z)).to_degrees();
        self.steering = steering(self.angle);
    }
}

/// Linear from the dead zone to full lock.
fn steering(angle: f32) -> f32 {
    let beyond = (angle.abs() - DEAD_ZONE_DEGREES).max(0.0);
    (beyond / (FULL_LOCK_DEGREES - DEAD_ZONE_DEGREES))
        .min(1.0)
        .copysign(angle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gravity for a phone held upright in landscape-left (its top to the
    /// right, so the device's x axis points down), screen facing the player,
    /// turned `degrees` to the right like a wheel: the right end goes down.
    fn turned(degrees: f32) -> [f32; 3] {
        let radians = degrees.to_radians();
        [libm::cosf(radians), libm::sinf(radians), 0.0]
    }

    #[test]
    fn level_goes_straight_and_25_degrees_is_full_lock() {
        let mut wheel = Wheel::default();
        wheel.update(turned(0.0), true);
        assert_eq!(wheel.steering, 0.0);
        wheel.update(turned(2.0), true);
        assert_eq!(wheel.steering, 0.0, "inside the dead zone");
        wheel.update(turned(13.75), true);
        assert!((wheel.steering - 0.5).abs() < 1e-3, "{}", wheel.steering);
        wheel.update(turned(-40.0), true);
        assert_eq!(wheel.steering, -1.0);
    }

    #[test]
    fn the_other_landscape_steers_the_same_way() {
        let (mut left, mut right) = (Wheel::default(), Wheel::default());
        left.update(turned(10.0), true);
        // Turned the other way round, the device's axes point the other way.
        let [x, y, z] = turned(10.0);
        right.update([-x, -y, z], false);
        assert!((left.steering - right.steering).abs() < 1e-5);
    }

    #[test]
    fn holding_the_phone_flatter_does_not_change_the_steering() {
        let mut upright = Wheel::default();
        upright.update(turned(10.0), true);
        // Same wheel angle, screen tilted 60° back toward the sky.
        let [x, y, _] = turned(10.0);
        let back = 60.0_f32.to_radians();
        let mut flatter = Wheel::default();
        flatter.update([x * libm::cosf(back), y, x * libm::sinf(back)], true);
        assert!(
            (upright.angle - flatter.angle).abs() < 0.01,
            "{} vs {}",
            upright.angle,
            flatter.angle
        );
    }

    #[test]
    fn a_phone_lying_flat_keeps_the_last_steering() {
        let mut wheel = Wheel::default();
        wheel.update(turned(15.0), true);
        let before = wheel.steering;
        wheel.update([0.05, 0.1, -1.0], true);
        assert_eq!(wheel.steering, before);
    }
}
