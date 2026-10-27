//! Stage 1 of `CERTIFICATION.md` for the wire format: every message comes
//! back from the wire as it left, and any frame at all is read as a message
//! or refused, quickly, never with a panic. `PROPTEST_CASES=5000 cargo test`
//! searches further than the default 256 cases.

#![expect(
    clippy::arithmetic_side_effects,
    reason = "test: an overflow must panic and fail the test"
)]

mod strategies;

use std::time::{Duration, Instant};

use proptest::collection::vec;
use proptest::prelude::*;
use remote_protocol::{
    Anchor, Control, GameMessage, InputState, Kind, Label, Layout, PhoneMessage, Place, Remote,
    decode, encode,
};
use strategies::{Damage, HUGE_VARINT, damage, game_message, phone_message};

/// A frame decodes as `T` or is refused; what decodes encodes back to a
/// frame that decodes to the same message.
fn read_back<T>(frame: &[u8]) -> Option<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let message = decode::<T>(frame).ok()?;
    let again = decode::<T>(&encode(&message)).ok();
    assert_eq!(
        format!("{again:?}"),
        format!("{:?}", Some(&message)),
        "a decoded message does not survive the wire"
    );
    Some(message)
}

fn damaged(mut frame: Vec<u8>, damages: &[Damage]) -> Vec<u8> {
    for damage in damages {
        damage.apply(&mut frame);
    }
    frame
}

proptest! {
    /// E1: every message a phone can send comes back from the wire exactly
    /// as it left, NaN and negative zero included (compared by their
    /// encoding and their debug form).
    #[test]
    fn every_phone_message_survives_the_wire(message in phone_message()) {
        let frame = encode(&message);
        prop_assert!(!frame.is_empty());
        let back = decode::<PhoneMessage>(&frame);
        prop_assert_eq!(format!("{back:?}"), format!("{:?}", Ok::<_, ()>(&message)));
        prop_assert_eq!(back.map(|m| encode(&m)), Ok(frame));
    }

    /// E1: the same for every message a game can send, hostile layouts
    /// included.
    #[test]
    fn every_game_message_survives_the_wire(message in game_message()) {
        let frame = encode(&message);
        prop_assert!(!frame.is_empty());
        let back = decode::<GameMessage>(&frame);
        prop_assert_eq!(format!("{back:?}"), format!("{:?}", Ok::<_, ()>(&message)));
        prop_assert_eq!(back.map(|m| encode(&m)), Ok(frame));
    }

    /// E1: random bytes are refused or read as a message, on both sides,
    /// never a panic.
    #[test]
    fn any_bytes_are_a_message_or_refused(frame in vec(any::<u8>(), 0..256)) {
        read_back::<PhoneMessage>(&frame);
        read_back::<GameMessage>(&frame);
    }

    /// E1: a real frame damaged on its way (cut, a byte flipped, inserted or
    /// lost, a length claiming 2^64 elements) is refused or read as a
    /// message, never a panic.
    #[test]
    fn damaged_frames_are_a_message_or_refused(
        phone in phone_message(),
        game in game_message(),
        damages in vec(damage(), 1..4),
    ) {
        for frame in [encode(&phone), encode(&game)] {
            let frame = damaged(frame, &damages);
            read_back::<PhoneMessage>(&frame);
            read_back::<GameMessage>(&frame);
        }
    }
}

/// The bytes of `message` before one of its lengths, found as an empty list
/// or text followed by `after` more bytes.
fn before_length<T: serde::Serialize>(message: &T, after: usize) -> Vec<u8> {
    let mut frame = encode(message);
    frame.truncate(frame.len() - after);
    assert_eq!(frame.pop(), Some(0), "not an empty length");
    frame
}

/// E1: a length claiming more elements than any memory holds fails at once,
/// instead of reserving room for them: for every list and text of both
/// directions, with nothing, 64 KiB of zeros or 64 KiB of ones behind it.
/// An absurd allocation would abort the process or take seconds.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test measures real time; no simulation is involved"
)]
fn a_huge_length_fails_at_once() {
    const U32_MAX_VARINT: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
    let phone_prefixes = [
        before_length(
            &PhoneMessage::Hello {
                version: 1,
                player: [7; 16],
                name: String::new(),
            },
            0,
        ),
        // The sticks, before tilt and pointer.
        before_length(&PhoneMessage::Input(InputState::default()), 2),
    ];
    let pad = |controls| {
        GameMessage::Show(Remote::Pad(Layout {
            controls,
            ..Layout::default()
        }))
    };
    let labelled = Control {
        id: 0,
        kind: Kind::DPad { size: 100.0 },
        at: Place::new(Anchor::Center, 0.0, 0.0),
        label: Label::Text(String::new()),
        color: 0,
    };
    let game_prefixes = [
        // The controls, before tilt, pointer and canvas.
        before_length(&pad(Vec::new()), 3),
        before_length(&GameMessage::Show(Remote::Web { url: String::new() }), 0),
        before_length(
            &GameMessage::Refuse {
                reason: String::new(),
            },
            0,
        ),
        // A label's text, before the color, tilt, pointer and canvas.
        before_length(&pad(vec![labelled]), 4),
    ];
    let tails = [vec![], vec![0; 64 * 1024], vec![1; 64 * 1024]];
    let start = Instant::now();
    for length in [&HUGE_VARINT[..], &U32_MAX_VARINT] {
        for tail in &tails {
            for prefix in &phone_prefixes {
                let frame = [prefix, length, tail].concat();
                assert!(decode::<PhoneMessage>(&frame).is_err(), "{prefix:?}");
            }
            for prefix in &game_prefixes {
                let frame = [prefix, length, tail].concat();
                assert!(decode::<GameMessage>(&frame).is_err(), "{prefix:?}");
            }
        }
    }
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}
