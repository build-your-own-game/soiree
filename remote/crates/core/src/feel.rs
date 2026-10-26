//! How the remotes feel under the thumbs, tested on every preset and every
//! iPhone, and in detail on the N64 pad, which has every kind of control.

use remote_protocol::presets::{self, n64};

use crate::controls::{ButtonId, Layout, TouchPad};
use crate::draw::canvas_rect;
use crate::tests::{PHONES, assert_fits};
use crate::{Haptic, Point};

fn pad(spec: &remote_protocol::Layout) -> TouchPad {
    let (screen, insets) = PHONES[0];
    TouchPad::new(Layout::resolve(spec, screen, insets))
}

fn center_of(pad: &TouchPad, id: ButtonId) -> Point {
    pad.layout()
        .buttons
        .iter()
        .find(|b| b.id == id)
        .map(|b| b.center)
        .unwrap_or_default()
}

fn stick_center(pad: &TouchPad) -> Point {
    pad.layout()
        .sticks
        .first()
        .map(|s| s.center)
        .unwrap_or_default()
}

fn travel(pad: &TouchPad) -> f32 {
    pad.layout().sticks.first().map_or(0.0, |s| s.travel)
}

fn held(pad: &TouchPad) -> Vec<ButtonId> {
    pad.pressed().ids().collect()
}

/// The first stick's value, scaled like a real N64 stick (85 at the gate).
fn n64_stick(pad: &TouchPad) -> (i32, i32) {
    let value = pad.sticks().first().map(|(_, v)| *v).unwrap_or_default();
    #[expect(clippy::cast_possible_truncation, reason = "test values within ±85")]
    let scale = |v: f32| (v * 85.0).round() as i32;
    (scale(value.x), scale(value.y))
}

#[test]
fn every_preset_fits_every_phone() {
    for (name, spec) in presets::all() {
        for (screen, insets) in PHONES {
            let layout = Layout::resolve(&spec, screen, insets);
            assert_eq!(
                layout.buttons.len() + 4 * layout.dpads.len() + layout.sticks.len(),
                {
                    spec.controls
                        .iter()
                        .map(|c| {
                            if matches!(c.kind, remote_protocol::Kind::DPad { .. }) {
                                4
                            } else {
                                1
                            }
                        })
                        .sum::<usize>()
                },
                "{name}: a control was left out"
            );
            assert_fits(&layout);
            if spec.canvas {
                let canvas = canvas_rect(layout.area);
                for b in &layout.buttons {
                    assert!(
                        !b.shape.bounds(b.center).overlaps(canvas),
                        "{name}: tool {} covers the canvas",
                        b.id
                    );
                }
            }
        }
    }
}

#[test]
fn every_button_is_pressed_alone_at_its_center() {
    for (name, spec) in presets::all() {
        for (screen, insets) in PHONES {
            let mut pad = TouchPad::new(Layout::resolve(&spec, screen, insets));
            let buttons = pad.layout().buttons.clone();
            for (touch, b) in (0..).zip(&buttons) {
                assert_eq!(
                    pad.touch_down(touch, b.center),
                    Some(Haptic::Press),
                    "{name}"
                );
                assert_eq!(held(&pad), [b.id], "{name}");
                assert_eq!(pad.touch_up(touch), Some(Haptic::Release));
            }
        }
    }
}

#[test]
fn the_buzzer_answers_anywhere() {
    let mut pad = pad(&presets::buzzer());
    let area = pad.layout().area;
    for (touch, corner) in (0..).zip([
        Point::new(area.left + 1.0, area.top + 1.0),
        Point::new(area.right - 1.0, area.bottom - 1.0),
    ]) {
        pad.touch_down(touch, corner);
        assert_eq!(held(&pad), [presets::buzzer::BUZZ]);
        pad.touch_up(touch);
    }
}

#[test]
fn one_thumb_between_a_and_b_presses_both() {
    let mut pad = pad(&presets::n64());
    let (a, b) = (center_of(&pad, n64::A), center_of(&pad, n64::B));
    pad.touch_down(1, Point::new(a.x.midpoint(b.x), a.y.midpoint(b.y)));
    assert_eq!(held(&pad), [n64::A, n64::B]);
}

#[test]
fn one_thumb_between_two_c_buttons_presses_one() {
    let mut pad = pad(&presets::n64());
    let (up, right) = (center_of(&pad, n64::C_UP), center_of(&pad, n64::C_RIGHT));
    pad.touch_down(
        1,
        Point::new(up.x.midpoint(right.x), up.y.midpoint(right.y)),
    );
    assert_eq!(held(&pad).len(), 1);
}

#[test]
fn sliding_from_b_to_a_rolls_through_both() {
    let mut pad = pad(&presets::n64());
    let (a, b) = (center_of(&pad, n64::A), center_of(&pad, n64::B));
    assert_eq!(pad.touch_down(1, b), Some(Haptic::Press));
    assert_eq!(
        pad.touch_move(1, Point::new(a.x.midpoint(b.x), a.y.midpoint(b.y))),
        Some(Haptic::Press)
    );
    assert_eq!(
        held(&pad),
        [n64::A, n64::B],
        "B stays held while rolling onto A"
    );
    assert_eq!(pad.touch_move(1, a), Some(Haptic::Release));
    assert_eq!(held(&pad), [n64::A]);
    let below = Point::new(a.x - 45.0, pad.layout().area.bottom - 2.0);
    assert_eq!(pad.touch_move(1, below), Some(Haptic::Release));
    assert_eq!(held(&pad), [], "between buttons nothing is held");
}

#[test]
fn a_held_button_keeps_a_wider_margin() {
    let mut pad = pad(&presets::n64());
    let a = center_of(&pad, n64::A);
    let edge = Point::new(a.x + 40.0 + 25.0, a.y);
    pad.touch_down(1, edge);
    assert_eq!(held(&pad), [], "25 pt off A is too far for a new touch");
    pad.touch_down(2, a);
    pad.touch_move(2, edge);
    assert_eq!(held(&pad), [n64::A], "but not for a finger already on A");
}

#[test]
fn minor_buttons_take_no_touch_beyond_their_drawing() {
    let mut pad = pad(&presets::cross());
    let start = center_of(&pad, presets::cross::START);
    pad.touch_down(1, Point::new(start.x, start.y - 20.0));
    assert_eq!(held(&pad), [], "just above Start");
}

#[test]
fn two_thumbs_hold_two_buttons_and_lifting_one_keeps_the_other() {
    let mut pad = pad(&presets::n64());
    pad.touch_down(1, center_of(&pad, n64::L));
    pad.touch_down(2, center_of(&pad, n64::A));
    assert_eq!(held(&pad), [n64::A, n64::L]);
    assert_eq!(pad.touch_up(1), Some(Haptic::Release));
    assert_eq!(held(&pad), [n64::A]);
}

#[test]
fn stick_centers_where_the_thumb_lands_near_its_rest() {
    let mut pad = pad(&presets::n64());
    let rest = stick_center(&pad);
    let landing = Point::new(rest.x + 20.0, rest.y - 15.0);
    assert_eq!(pad.touch_down(1, landing), None);
    assert_eq!(
        n64_stick(&pad),
        (0, 0),
        "landing off-center does not deflect"
    );
    assert_eq!(
        pad.knobs(),
        [(n64::STICK, rest)],
        "the hidden recentering does not move the drawing"
    );
    pad.touch_move(1, Point::new(landing.x + 10.0, landing.y));
    assert_eq!(
        pad.knobs(),
        [(n64::STICK, Point::new(rest.x + 10.0, rest.y))]
    );
}

#[test]
fn a_thumb_landing_far_from_the_stick_already_pushes_it() {
    let mut pad = pad(&presets::n64());
    let area = pad.layout().area;
    pad.touch_down(1, Point::new(area.left + 10.0, area.bottom - 5.0));
    let (x, y) = n64_stick(&pad);
    assert!(x < -60 && y < -10, "leans down-left at once: {x}, {y}");
}

#[test]
fn the_whole_left_half_takes_the_stick() {
    let mut pad = pad(&presets::n64());
    let area = pad.layout().area;
    pad.touch_down(
        1,
        Point::new(area.center_x() - 10.0, area.top + area.height() * 0.6),
    );
    assert!(n64_stick(&pad).0 > 0);
    pad.touch_up(1);
    pad.touch_down(
        2,
        Point::new(area.center_x() + 10.0, area.top + area.height() * 0.6),
    );
    assert_eq!(n64_stick(&pad), (0, 0), "the right half does not");
}

#[test]
fn stick_reads_up_as_positive_and_stops_in_the_n64_gate() {
    let mut pad = pad(&presets::n64());
    let (origin, travel) = (stick_center(&pad), travel(&pad));
    pad.touch_down(1, origin);
    pad.touch_move(1, Point::new(origin.x, origin.y - travel * 0.5));
    assert_eq!(
        n64_stick(&pad),
        (0, 41),
        "half travel reads about half of 85"
    );

    assert_eq!(
        pad.touch_move(1, Point::new(origin.x + travel * 3.0, origin.y)),
        Some(Haptic::Edge)
    );
    assert_eq!(n64_stick(&pad), (85, 0));
    assert_eq!(
        pad.knobs(),
        [(n64::STICK, Point::new(origin.x + travel, origin.y))],
        "the knob stops at the gate"
    );

    let diagonal = travel * 2.0;
    pad.touch_move(1, Point::new(origin.x - diagonal, origin.y + diagonal));
    assert_eq!(
        n64_stick(&pad),
        (-70, -70),
        "the diagonal notch of a real gate, not 85/√2 = 60"
    );
}

#[test]
fn diagonal_rim_is_further_than_the_axis_rim() {
    let mut pad = pad(&presets::n64());
    let (origin, travel) = (stick_center(&pad), travel(&pad));
    pad.touch_down(1, origin);
    let step = travel * std::f32::consts::FRAC_1_SQRT_2;
    assert_eq!(
        pad.touch_move(1, Point::new(origin.x + step, origin.y - step)),
        None,
        "not yet the gate"
    );
    let (x, y) = n64_stick(&pad);
    assert!(x == y && (58..=62).contains(&x), "{x}, {y}");
    assert_eq!(
        pad.touch_move(1, Point::new(origin.x + step * 1.2, origin.y - step * 1.2)),
        Some(Haptic::Edge)
    );
}

#[test]
fn stick_dead_zone_only_filters_jitter() {
    let mut pad = pad(&presets::pad());
    let (origin, travel) = (stick_center(&pad), travel(&pad));
    pad.touch_down(1, origin);
    pad.touch_move(1, Point::new(origin.x + travel * 0.02, origin.y));
    assert_eq!(pad.sticks(), [(presets::pad::STICK, Point::default())]);
    pad.touch_move(1, Point::new(origin.x + travel * 0.1, origin.y));
    let x = pad.sticks().first().map_or(0.0, |(_, v)| v.x);
    assert!(
        (0.06..=0.08).contains(&x),
        "no jump past the dead zone: {x}"
    );
}

#[test]
fn rim_haptic_fires_once_until_the_stick_comes_back() {
    let mut pad = pad(&presets::pad());
    let (origin, travel) = (stick_center(&pad), travel(&pad));
    pad.touch_down(1, origin);
    let at = |f: f32| Point::new(origin.x + travel * f, origin.y);
    assert_eq!(pad.touch_move(1, at(1.0)), Some(Haptic::Edge));
    assert_eq!(pad.touch_move(1, at(1.5)), None);
    assert_eq!(
        pad.touch_move(1, at(0.9)),
        None,
        "inside the hysteresis band"
    );
    assert_eq!(pad.touch_move(1, at(1.0)), None);
    pad.touch_move(1, at(0.5));
    assert_eq!(pad.touch_move(1, at(1.0)), Some(Haptic::Edge));
}

#[test]
fn a_thumb_that_leaves_the_stick_keeps_steering_and_presses_nothing() {
    let mut pad = pad(&presets::n64());
    pad.touch_down(1, stick_center(&pad));
    pad.touch_move(1, center_of(&pad, n64::A));
    assert_eq!(held(&pad), []);
    assert!(n64_stick(&pad).0 > 70);
}

#[test]
fn a_second_thumb_on_the_stick_side_is_ignored() {
    let mut pad = pad(&presets::n64());
    let origin = stick_center(&pad);
    pad.touch_down(1, origin);
    pad.touch_down(2, Point::new(origin.x + 10.0, origin.y));
    pad.touch_move(2, Point::new(origin.x + 200.0, origin.y));
    assert_eq!(n64_stick(&pad), (0, 0));
}

#[test]
fn dpad_favors_straight_directions() {
    let mut pad = pad(&presets::n64());
    let dpad = pad.layout().dpads.first().copied();
    let Some(dpad) = dpad else {
        unreachable!("the N64 pad has a D-pad");
    };
    let [up, down, left, right] = dpad.ids;
    let r = dpad.size * 0.4;
    let cases = [
        (0.0, -1.0, vec![up]),
        (1.0, -1.0, vec![up, right]),
        (1.0, -0.45, vec![right]),
        (1.0, 0.0, vec![right]),
        (1.0, 1.0, vec![down, right]),
        (0.0, 1.0, vec![down]),
        (-0.45, 1.0, vec![down]),
        (-1.0, 0.0, vec![left]),
    ];
    pad.touch_down(1, dpad.center);
    assert_eq!(held(&pad), [], "the center of the cross is neutral");
    for (dx, dy, mut expected) in cases {
        pad.touch_move(
            1,
            Point::new(dpad.center.x + dx * r, dpad.center.y + dy * r),
        );
        expected.sort_unstable();
        assert_eq!(held(&pad), expected, "direction ({dx}, {dy})");
    }
}

#[test]
fn a_new_layout_lets_go_of_everything() {
    let mut pad = pad(&presets::n64());
    pad.touch_down(1, center_of(&pad, n64::A));
    let (screen, insets) = PHONES[1];
    pad.set_layout(Layout::resolve(&presets::n64(), screen, insets));
    assert_eq!(held(&pad), []);
    pad.touch_move(1, center_of(&pad, n64::A));
    assert_eq!(
        held(&pad),
        [],
        "a finger from before the change is forgotten"
    );
}

#[test]
fn touches_on_empty_space_do_nothing() {
    let mut pad = pad(&presets::n64());
    let area = pad.layout().area;
    let empty = Point::new(area.center_x() + 5.0, area.top + area.height() * 0.45);
    assert_eq!(pad.touch_down(1, empty), None);
    pad.touch_move(1, center_of(&pad, n64::A));
    assert_eq!(held(&pad), []);
}
