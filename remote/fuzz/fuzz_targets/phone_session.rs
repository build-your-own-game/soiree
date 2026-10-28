//! The phone's session driven by a hostile game and a busy player: any frames
//! from the game (garbage, or layouts nobody designed), any touches, motion
//! and screen changes. Never a panic, every frame it sends decodes, every
//! knob stays a number, and its strokes are a `Begin`, its points and an
//! `End`, one at a time, ended before the game's next remote or page.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use remote_core::remote::Motion;
use remote_core::session::{Event, Session};
use remote_core::{Insets, Orientation, Point, Size};
use remote_protocol::{DrawEvent, GameMessage, PhoneMessage, Remote, decode, encode, presets};

#[derive(Arbitrary, Debug)]
enum Op {
    /// Raw bytes from the game.
    Frame(Vec<u8>),
    /// A well-formed layout: a preset, so the touches below land on something.
    Preset(u8),
    Resize {
        width: u16,
        height: u16,
        insets: [u8; 4],
    },
    Down {
        id: u8,
        x: i16,
        y: i16,
    },
    Move {
        id: u8,
        x: i16,
        y: i16,
    },
    Up {
        id: u8,
    },
    Motion {
        gravity: [f32; 3],
        rotation_rate: [f32; 3],
        landscape_left: bool,
    },
    ReleaseAll,
    /// Time passes, in milliseconds.
    Wait(u16),
    Connected,
}

fuzz_target!(|ops: Vec<Op>| {
    let screen = Size {
        width: 390.0,
        height: 844.0,
    };
    let mut session = Session::new([1; 16], "fuzz".into(), screen, Insets::default());
    session.connected();
    let mut now = 0.0_f64;
    // The stroke being drawn, as the game sees it.
    let mut open = None;
    for op in ops {
        let mut shown = None;
        match op {
            Op::Frame(frame) => shown = Some(session.receive(&frame)),
            Op::Preset(index) => {
                let all = presets::all();
                let (_, layout) = all[usize::from(index) % all.len()].clone();
                shown = Some(session.receive(&encode(&GameMessage::Show(Remote::Pad(layout)))));
            }
            Op::Resize {
                width,
                height,
                insets: [top, left, bottom, right],
            } => {
                let size = Size {
                    width: f32::from(width % 3000),
                    height: f32::from(height % 3000),
                };
                let insets = Insets {
                    top: f32::from(top),
                    left: f32::from(left),
                    bottom: f32::from(bottom),
                    right: f32::from(right),
                };
                session.resize(size, insets);
            }
            Op::Down { id, x, y } => {
                if let Some(remote) = session.remote_mut() {
                    remote.touch_down(u64::from(id), point(x, y), now);
                }
            }
            Op::Move { id, x, y } => {
                if let Some(remote) = session.remote_mut() {
                    remote.touch_move(u64::from(id), point(x, y), now);
                }
            }
            Op::Up { id } => {
                if let Some(remote) = session.remote_mut() {
                    remote.touch_up(u64::from(id));
                }
            }
            Op::Motion {
                gravity,
                rotation_rate,
                landscape_left,
            } => {
                if let Some(remote) = session.remote_mut() {
                    remote.motion(Motion {
                        gravity,
                        rotation_rate,
                        time: now,
                        orientation: if landscape_left {
                            Orientation::LandscapeLeft
                        } else {
                            Orientation::LandscapeRight
                        },
                    });
                }
            }
            Op::ReleaseAll => {
                if let Some(remote) = session.remote_mut() {
                    remote.release_all();
                }
            }
            Op::Wait(ms) => now += f64::from(ms) / 1000.0,
            Op::Connected => session.connected(),
        }
        session.tick(now);
        if let Some(remote) = session.remote() {
            let _ = remote.selected(now);
            let _ = remote.layout();
            for (id, knob) in remote.knobs() {
                assert!(
                    knob.x.is_finite() && knob.y.is_finite(),
                    "knob {id:?} at {knob:?}"
                );
            }
        }
        for frame in session.drain() {
            let message = decode::<PhoneMessage>(&frame).expect("the phone sends what it can read");
            if let PhoneMessage::Draw(event) = message {
                follow_stroke(&mut open, event);
            }
        }
        if matches!(shown, Some(Event::ShowPad | Event::ShowWeb(_))) {
            assert_eq!(
                open, None,
                "a stroke cut by the game's next remote or page never ended"
            );
        }
    }
});

fn follow_stroke(open: &mut Option<u16>, event: DrawEvent) {
    match event {
        DrawEvent::Begin { stroke, .. } => {
            assert_eq!(
                *open, None,
                "stroke {stroke} began before the last one ended"
            );
            *open = Some(stroke);
        }
        DrawEvent::Point { stroke, .. } => assert_eq!(*open, Some(stroke), "a stray point"),
        DrawEvent::End { stroke } => assert_eq!(open.take(), Some(stroke), "a stray end"),
        DrawEvent::Undo | DrawEvent::Clear => {}
    }
}

fn point(x: i16, y: i16) -> Point {
    Point {
        x: f32::from(x) / 10.0,
        y: f32::from(y) / 10.0,
    }
}
