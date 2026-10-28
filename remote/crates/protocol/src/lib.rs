//! The contract between a game and the iPhone remote, as `MANIFESTE.md`
//! describes it (*Télécommande native*).
//!
//! The game serves a WebSocket at [`PATH`]. Each binary frame carries one
//! message, encoded with [`encode`] and read with [`decode`]. The phone says
//! [`PhoneMessage::Hello`]; the game answers [`GameMessage::Show`] with the
//! remote to display, and sends it again to change it. The phone then sends
//! its state ([`InputState`]) and drawing strokes ([`DrawEvent`]).

use serde::{Deserialize, Serialize};

mod layout;
pub mod presets;
pub mod web;

pub use layout::{
    Action, Anchor, Control, Direction, Gate, Kind, Label, Layout, MAX_ID, Place, Shape,
};

/// Bumped whenever a message changes shape. A game refuses another version.
pub const VERSION: u16 = 1;

/// Where the game serves the remote's WebSocket, beside its phone page.
pub const PATH: &str = "/remote";

/// Phone to game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PhoneMessage {
    /// First message, and again after each reconnection.
    Hello {
        version: u16,
        /// The player's identity, the same on every reconnection: random,
        /// kept by the app, no account.
        player: [u8; 16],
        /// The phone owner's first name, if the app knows it.
        name: String,
    },
    /// The whole state of the remote's controls. The latest one wins.
    Input(InputState),
    /// A drawing, step by step, in order.
    Draw(DrawEvent),
}

/// Game to phone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GameMessage {
    /// The remote to show now. Sent again to change it.
    Show(Remote),
    /// Something to feel or see, for instance "first on the buzzer".
    Feedback(Feedback),
    /// The game will not take this phone; the app shows the reason.
    Refuse { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Remote {
    /// Controls the app draws and handles itself, with its touch rules: a
    /// preset from [`presets`] or the game's own composition.
    Pad(Layout),
    /// The game's own page, full screen. A relative `url` starts from the
    /// game's host. The page talks to the game however it likes.
    Web { url: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub haptic: Option<Haptic>,
    /// Tints the remote's background, `0xRRGGBB`, until the next feedback.
    pub color: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Haptic {
    Press,
    Release,
    Success,
    Warning,
}

/// The state of a [`Remote::Pad`]: what a game reads, whole, each time.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputState {
    /// Counts up with every state sent: a game ignores an older one.
    pub seq: u32,
    /// One bit per pressed button or D-pad direction, by control id.
    pub pressed: u32,
    /// Every stick, even at rest.
    pub sticks: Vec<StickValue>,
    /// Steering from tilting the phone, -127 (left) to 127 (right), for a
    /// layout with `tilt`.
    pub tilt: i8,
    /// Where the cursor is on the TV, 0 to 65535 across and down, for a
    /// layout with `pointer`.
    pub pointer: Option<[u16; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickValue {
    pub id: u8,
    /// -127 to 127, Y up.
    pub x: i8,
    pub y: i8,
}

impl InputState {
    pub fn is_pressed(&self, id: u8) -> bool {
        self.pressed
            .checked_shr(u32::from(id))
            .is_some_and(|bits| bits & 1 == 1)
    }

    /// The stick's deflection on each axis, -1 to 1, Y up; 0 if absent.
    pub fn stick(&self, id: u8) -> (f32, f32) {
        self.sticks
            .iter()
            .find(|s| s.id == id)
            .map_or((0.0, 0.0), |s| (unit_i8(s.x), unit_i8(s.y)))
    }

    /// Steering, -1 to 1.
    pub fn tilt(&self) -> f32 {
        unit_i8(self.tilt)
    }

    /// The cursor, 0 to 1 across and down.
    pub fn pointer(&self) -> Option<(f32, f32)> {
        self.pointer
            .map(|[x, y]| (f32::from(x) / 65535.0, f32::from(y) / 65535.0))
    }
}

fn unit_i8(value: i8) -> f32 {
    f32::from(value) / 127.0
}

/// -1..=1 to -127..=127.
#[expect(
    clippy::cast_possible_truncation,
    reason = "rounded and clamped to ±127 first"
)]
pub fn to_i8(value: f32) -> i8 {
    (value * 127.0).round().clamp(-127.0, 127.0) as i8
}

/// 0..=1 to 0..=65535.
#[expect(
    clippy::cast_possible_truncation,
    reason = "rounded and clamped to u16 first"
)]
#[expect(clippy::cast_sign_loss, reason = "clamped to 0 first")]
pub fn to_u16(value: f32) -> u16 {
    (value * 65535.0).round().clamp(0.0, 65535.0) as u16
}

/// One step of a drawing, in canvas units: 4096 across, 2304 down (16:9, as
/// the TV). A stroke may run past the edge. A game rebuilds the drawing from
/// these, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawEvent {
    /// `color` is `0xRRGGBB`; `width` in canvas units.
    Begin {
        stroke: u16,
        color: u32,
        width: u16,
        x: i16,
        y: i16,
    },
    Point {
        stroke: u16,
        x: i16,
        y: i16,
    },
    End {
        stroke: u16,
    },
    /// Removes the last finished stroke.
    Undo,
    Clear,
}

pub const CANVAS_WIDTH: u16 = 4096;
pub const CANVAS_HEIGHT: u16 = 2304;

pub use postcard::Error as DecodeError;

/// One message, ready for a binary WebSocket frame.
pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    // Serializing these types cannot fail: no maps, no unsized lengths.
    postcard::to_allocvec(message).unwrap_or_default()
}

pub fn decode<'a, T: Deserialize<'a>>(frame: &'a [u8]) -> Result<T, DecodeError> {
    salon_core::codec::decode(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_decoders_reject_concatenated_or_extended_frames() {
        let mut phone = encode(&PhoneMessage::Input(InputState::default()));
        phone.extend(encode(&PhoneMessage::Draw(DrawEvent::Clear)));
        assert!(decode::<PhoneMessage>(&phone).is_err());
        let mut game = encode(&GameMessage::Refuse {
            reason: "full".into(),
        });
        game.push(0);
        assert!(decode::<GameMessage>(&game).is_err());
    }

    #[test]
    fn every_message_survives_the_wire() {
        let phone = [
            PhoneMessage::Hello {
                version: VERSION,
                player: [7; 16],
                name: "Axel".into(),
            },
            PhoneMessage::Input(InputState {
                seq: 42,
                pressed: 0b101,
                sticks: vec![StickValue {
                    id: 3,
                    x: -127,
                    y: 64,
                }],
                tilt: -12,
                pointer: Some([0, 65535]),
            }),
            PhoneMessage::Draw(DrawEvent::Begin {
                stroke: 1,
                color: 0x00FF_0000,
                width: 41,
                x: -5,
                y: 2400,
            }),
            PhoneMessage::Draw(DrawEvent::Clear),
        ];
        for message in phone {
            assert_eq!(decode::<PhoneMessage>(&encode(&message)), Ok(message));
        }
        let game = [
            GameMessage::Show(Remote::Pad(presets::n64())),
            GameMessage::Show(Remote::Web {
                url: "/play/remote".into(),
            }),
            GameMessage::Feedback(Feedback {
                haptic: Some(Haptic::Success),
                color: Some(0x0000_FF00),
            }),
            GameMessage::Refuse {
                reason: "La partie est pleine".into(),
            },
        ];
        for message in game {
            assert_eq!(decode::<GameMessage>(&encode(&message)), Ok(message));
        }
    }

    #[test]
    fn a_state_reads_back_in_units() {
        let state = InputState {
            pressed: 1 << 4,
            sticks: vec![StickValue {
                id: 2,
                x: 127,
                y: -127,
            }],
            tilt: to_i8(-0.5),
            pointer: Some([to_u16(0.25), to_u16(1.0)]),
            ..InputState::default()
        };
        assert!(state.is_pressed(4) && !state.is_pressed(3) && !state.is_pressed(200));
        assert_eq!(state.stick(2), (1.0, -1.0));
        assert_eq!(state.stick(9), (0.0, 0.0));
        assert!((state.tilt() + 0.5).abs() < 0.01);
        let (x, y) = state.pointer().unwrap_or_default();
        assert!((x - 0.25).abs() < 1e-4 && (y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn garbage_is_an_error_not_a_crash() {
        assert!(decode::<GameMessage>(&[0xFF, 0x13, 0x37]).is_err());
        assert!(decode::<PhoneMessage>(&[]).is_err());
    }
}
