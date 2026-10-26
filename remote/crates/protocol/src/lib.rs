//! What a remote sends to a game. Draft: only the Nintendo 64 pad exists yet.
//!
//! Inputs are state, not events: a remote sends its whole current state and a
//! game keeps the latest one. A lost packet is then replaced by the next one.

/// A Nintendo 64 pad, laid out like the real controller's status reply on the
/// Joybus wire: two button bytes, then the stick as signed X and Y (Y up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct N64Pad {
    pub buttons: N64Buttons,
    pub stick_x: i8,
    pub stick_y: i8,
}

impl N64Pad {
    /// Stick reach of a good original pad in its octagonal gate: 85 straight
    /// along an axis, 70 on both axes in a diagonal notch (the "ideal OEM"
    /// reference of the mimi test ROM). Nintendo only guaranteed 70 to 74 on
    /// a new pad, and games accept anything up to ±127.
    pub const STICK_CARDINAL: i8 = 85;
    pub const STICK_DIAGONAL: i8 = 70;

    /// Size of [`N64Pad::to_bytes`].
    pub const WIRE_LEN: usize = 4;

    /// The real controller's byte order: buttons high byte first, then X, Y.
    pub fn to_bytes(self) -> [u8; Self::WIRE_LEN] {
        let [high, low] = self.buttons.0.to_be_bytes();
        [
            high,
            low,
            self.stick_x.to_be_bytes()[0],
            self.stick_y.to_be_bytes()[0],
        ]
    }

    pub fn from_bytes(bytes: [u8; Self::WIRE_LEN]) -> Self {
        let [high, low, x, y] = bytes;
        Self {
            buttons: N64Buttons(u16::from_be_bytes([high, low])),
            stick_x: i8::from_be_bytes([x]),
            stick_y: i8::from_be_bytes([y]),
        }
    }
}

/// Pressed buttons, one bit each, at the real controller's bit positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct N64Buttons(pub u16);

impl N64Buttons {
    pub fn contains(self, button: N64Button) -> bool {
        self.0 & button.bit() != 0
    }

    pub fn set(&mut self, button: N64Button, pressed: bool) {
        if pressed {
            self.0 |= button.bit();
        } else {
            self.0 &= !button.bit();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum N64Button {
    A,
    B,
    Z,
    Start,
    DUp,
    DDown,
    DLeft,
    DRight,
    L,
    R,
    CUp,
    CDown,
    CLeft,
    CRight,
}

impl N64Button {
    pub const ALL: [Self; 14] = [
        Self::A,
        Self::B,
        Self::Z,
        Self::Start,
        Self::DUp,
        Self::DDown,
        Self::DLeft,
        Self::DRight,
        Self::L,
        Self::R,
        Self::CUp,
        Self::CDown,
        Self::CLeft,
        Self::CRight,
    ];

    /// Bit in [`N64Buttons`], as libultra defines it. The high byte is A B Z
    /// Start Up Down Left Right. In the low byte, bit 7 is the reset flag a
    /// real pad raises while L+R+Start recalibrates it, and bit 6 is unused:
    /// a remote has nothing to recalibrate, both stay clear.
    pub const fn bit(self) -> u16 {
        match self {
            Self::A => 0x8000,
            Self::B => 0x4000,
            Self::Z => 0x2000,
            Self::Start => 0x1000,
            Self::DUp => 0x0800,
            Self::DDown => 0x0400,
            Self::DLeft => 0x0200,
            Self::DRight => 0x0100,
            Self::L => 0x0020,
            Self::R => 0x0010,
            Self::CUp => 0x0008,
            Self::CDown => 0x0004,
            Self::CLeft => 0x0002,
            Self::CRight => 0x0001,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip() {
        let mut buttons = N64Buttons::default();
        buttons.set(N64Button::A, true);
        buttons.set(N64Button::CLeft, true);
        let pad = N64Pad {
            buttons,
            stick_x: -80,
            stick_y: 127,
        };
        assert_eq!(pad.to_bytes(), [0x80, 0x02, 0xb0, 0x7f]);
        assert_eq!(N64Pad::from_bytes(pad.to_bytes()), pad);
    }

    #[test]
    fn every_button_has_its_own_bit() {
        let mut all = 0u16;
        for button in N64Button::ALL {
            assert_eq!(button.bit().count_ones(), 1);
            assert_eq!(all & button.bit(), 0, "{button:?} shares a bit");
            all |= button.bit();
        }
        assert_eq!(all & 0x00c0, 0, "reset and reserved bits stay clear");
    }
}
