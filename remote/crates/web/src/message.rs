//! The messages between a page and the app, as plain data: built and read
//! here, so they are tested without a browser. `CONTRAT.md` documents the
//! same shapes for pages written in JavaScript.

use serde::{Deserialize, Serialize};
use serde_json::json;

/// The name of the app's message handler, and the prefix of its events.
pub const NAME: &str = "telecommande";

/// A ready-made vibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vibration {
    Press,
    Release,
    Success,
    Warning,
}

/// One step of a haptic pattern of your own.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HapticEvent {
    /// From the start of the pattern, in seconds.
    pub time: f64,
    /// In seconds; 0 is a tap.
    pub duration: f64,
    /// 0 to 1.
    pub intensity: f64,
    /// 0 (round) to 1 (crisp).
    pub sharpness: f64,
}

impl HapticEvent {
    /// A tap at `time`.
    pub const fn tap(time: f64, intensity: f64, sharpness: f64) -> Self {
        Self {
            time,
            duration: 0.0,
            intensity,
            sharpness,
        }
    }
}

/// Which orientations the page accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Allowed {
    Landscape,
    Portrait,
    Any,
}

/// The interface's orientation now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Portrait,
    /// The phone's top to the right.
    LandscapeLeft,
    LandscapeRight,
}

/// What the app tells the page before its scripts run.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Info {
    /// The player's identity: 32 hex digits, the same in every game.
    pub player: String,
    /// The first name the player gave the app, or "".
    pub name: String,
    pub orientation: Orientation,
}

/// One motion sample, in the device's axes: x to the right of the screen
/// held in portrait, y to its top, z out of it.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Motion {
    /// In g.
    pub gravity: [f64; 3],
    /// Radians per second.
    pub rotation: [f64; 3],
    /// In g, gravity aside.
    pub acceleration: [f64; 3],
    /// A quaternion `[x, y, z, w]`.
    pub attitude: [f64; 4],
    /// Seconds.
    pub time: f64,
}

/// A datagram from the game's host.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Datagram {
    id: u32,
    /// Base64.
    data: String,
}

pub fn vibrate(vibration: Vibration) -> String {
    json!({ "vibrate": vibration }).to_string()
}

/// At most 64 events, as the app plays no more.
pub fn haptic(events: &[HapticEvent]) -> String {
    json!({ "haptic": events.iter().take(64).collect::<Vec<_>>() }).to_string()
}

pub fn motion(on: bool) -> String {
    json!({ "motion": on }).to_string()
}

pub fn orientation(allowed: Allowed) -> String {
    json!({ "orientation": allowed }).to_string()
}

pub fn open(id: u32, port: u16) -> String {
    json!({ "datagram": { "op": "open", "id": id, "port": port } }).to_string()
}

pub fn send(id: u32, data: &[u8]) -> String {
    json!({ "datagram": { "op": "send", "id": id, "data": data } }).to_string()
}

pub fn close(id: u32) -> String {
    json!({ "datagram": { "op": "close", "id": id } }).to_string()
}

pub fn read_info(json: &str) -> Option<Info> {
    serde_json::from_str(json).ok()
}

pub fn read_motion(json: &str) -> Option<Motion> {
    serde_json::from_str(json).ok()
}

pub fn read_orientation(json: &str) -> Option<Orientation> {
    serde_json::from_str(json).ok()
}

/// The datagram's socket id and bytes.
pub fn read_datagram(json: &str) -> Option<(u32, Vec<u8>)> {
    let datagram: Datagram = serde_json::from_str(json).ok()?;
    Some((datagram.id, base64(&datagram.data)?))
}

/// Standard base64, as the app encodes; `None` if it is not.
fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let digits = text.trim_end_matches('=').as_bytes();
    let mut bytes = Vec::with_capacity(digits.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &c in digits {
        buffer = (buffer << 6) | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap_or_default()
    }

    #[test]
    fn messages_have_the_documented_shapes() {
        assert_eq!(
            parsed(&vibrate(Vibration::Success)),
            json!({ "vibrate": "success" })
        );
        assert_eq!(parsed(&motion(true)), json!({ "motion": true }));
        assert_eq!(
            parsed(&orientation(Allowed::Portrait)),
            json!({ "orientation": "portrait" })
        );
        assert_eq!(
            parsed(&haptic(&[HapticEvent::tap(0.1, 1.0, 0.5)])),
            json!({ "haptic": [{ "time": 0.1, "duration": 0.0, "intensity": 1.0, "sharpness": 0.5 }] })
        );
        assert_eq!(
            parsed(&open(3, 8095)),
            json!({ "datagram": { "op": "open", "id": 3, "port": 8095 } })
        );
        assert_eq!(
            parsed(&send(3, b"hi")),
            json!({ "datagram": { "op": "send", "id": 3, "data": [104, 105] } })
        );
        assert_eq!(
            parsed(&close(3)),
            json!({ "datagram": { "op": "close", "id": 3 } })
        );
    }

    #[test]
    fn a_pattern_is_cut_at_64_events() {
        let long = vec![HapticEvent::tap(0.0, 1.0, 1.0); 100];
        let value = parsed(&haptic(&long));
        assert_eq!(value["haptic"].as_array().map(Vec::len), Some(64));
    }

    #[test]
    fn what_the_app_sends_reads_back() {
        let info = r#"{"player":"b17aad3e00000000000000000000beef","name":"Axel","orientation":"landscapeRight"}"#;
        assert_eq!(
            read_info(info),
            Some(Info {
                player: "b17aad3e00000000000000000000beef".into(),
                name: "Axel".into(),
                orientation: Orientation::LandscapeRight,
            })
        );
        let motion = r#"{"gravity":[0,-1,0],"rotation":[0.1,0,0],"acceleration":[0,0,0],"attitude":[0,0,0,1],"time":12.5}"#;
        assert_eq!(
            read_motion(motion).map(|m| (m.gravity, m.time)),
            Some(([0.0, -1.0, 0.0], 12.5))
        );
        assert_eq!(
            read_orientation(r#""portrait""#),
            Some(Orientation::Portrait)
        );
        assert_eq!(
            read_datagram(r#"{"id":2,"data":"w6ljaG8gOiBwaW5n"}"#),
            Some((2, "écho : ping".as_bytes().to_vec()))
        );
        assert_eq!(read_info("{}"), None);
        assert_eq!(read_datagram(r#"{"id":2,"data":"%%%"}"#), None);
    }

    #[test]
    fn base64_reads_every_padding() {
        for (text, bytes) in [
            ("", &b""[..]),
            ("YQ==", b"a"),
            ("YWI=", b"ab"),
            ("YWJj", b"abc"),
            ("/+8=", &[0xFF, 0xEF][..]),
        ] {
            assert_eq!(base64(text).as_deref(), Some(bytes), "{text}");
        }
    }
}
