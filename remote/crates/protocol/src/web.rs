//! The single JSON contract of a free page and the iPhone app. The page
//! uses this module directly; Swift uses it through the generated UniFFI
//! API. `CONTRAT.md` documents these same shapes for JavaScript authors.

use core::{fmt, str::FromStr};
use serde::{Deserialize, Serialize};

/// The name of the app's message handler, and the prefix of its events.
pub const NAME: &str = "telecommande";

/// A ready-made vibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vibration {
    Press,
    Release,
    Success,
    Warning,
}

/// One step of a haptic pattern of your own.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HapticEvent {
    /// From the start of the pattern, in seconds.
    #[serde(default)]
    pub time: f64,
    /// In seconds; 0 is a tap.
    #[serde(default)]
    pub duration: f64,
    /// 0 to 1.
    #[serde(default = "full_intensity")]
    pub intensity: f64,
    /// 0 (round) to 1 (crisp).
    #[serde(default = "medium_sharpness")]
    pub sharpness: f64,
}

const fn full_intensity() -> f64 {
    1.0
}
const fn medium_sharpness() -> f64 {
    0.5
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Allowed {
    Landscape,
    Portrait,
    Any,
}

/// The interface's orientation now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Portrait,
    /// The phone's top to the right.
    LandscapeLeft,
    LandscapeRight,
}

/// What the app tells the page before its scripts run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// The player's identity: 32 hex digits, the same in every game.
    pub player: PlayerId,
    /// The first name the player gave the app, or "".
    pub name: String,
    pub orientation: Orientation,
}

/// One motion sample, in the device's axes: x to the right of the screen
/// held in portrait, y to its top, z out of it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Datagram {
    id: u32,
    /// Base64.
    data: String,
}

/// Exactly sixteen bytes, represented by 32 hexadecimal digits in JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlayerId([u8; 16]);

impl PlayerId {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for PlayerId {
    type Err = &'static str;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 32 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("player must be 32 hex digits");
        }
        let mut bytes = [0; 16];
        for (byte, pair) in bytes
            .iter_mut()
            .zip(text.as_bytes().as_chunks::<2>().0.iter())
        {
            let text = std::str::from_utf8(pair).map_err(|_| "invalid player")?;
            *byte = u8::from_str_radix(text, 16).map_err(|_| "invalid player")?;
        }
        Ok(Self(bytes))
    }
}

impl TryFrom<String> for PlayerId {
    type Error = &'static str;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<PlayerId> for String {
    fn from(player: PlayerId) -> Self {
        player.to_string()
    }
}

/// One command, with the existing externally tagged JSON representation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Command {
    Vibrate(Vibration),
    Haptic(Vec<HapticEvent>),
    Motion(bool),
    Orientation(Allowed),
    Datagram(DatagramCommand),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum DatagramCommand {
    Open { id: u32, port: u16 },
    Send { id: u32, data: Vec<u8> },
    Close { id: u32 },
}

/// Old JavaScript pages can combine commands in one object. Preserve the
/// app's original order while validating every field at this boundary.
#[derive(Default, Deserialize)]
struct Commands {
    vibrate: Option<Vibration>,
    haptic: Option<Vec<HapticEvent>>,
    motion: Option<bool>,
    orientation: Option<Allowed>,
    datagram: Option<DatagramCommand>,
}

fn normalized(event: HapticEvent) -> HapticEvent {
    let bound = |value: f64, default: f64, max: f64| {
        if value.is_finite() {
            value.clamp(0.0, max)
        } else {
            default
        }
    };
    HapticEvent {
        time: bound(event.time, 0.0, 10.0),
        duration: bound(event.duration, 0.0, 5.0),
        intensity: bound(event.intensity, 1.0, 1.0),
        sharpness: bound(event.sharpness, 0.5, 1.0),
    }
}

pub fn read_commands(json: &str) -> Option<Vec<Command>> {
    // Serde also accepts positional arrays for structs. The WebKit contract
    // requires objects, including each haptic event, just like the old app.
    let fields: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).ok()?;
    if let Some(events) = fields.get("haptic").filter(|v| !v.is_null())
        && !events.as_array()?.iter().all(serde_json::Value::is_object)
    {
        return None;
    }
    let wire: Commands = serde_json::from_value(serde_json::Value::Object(fields)).ok()?;
    let mut commands = Vec::new();
    if let Some(v) = wire.vibrate {
        commands.push(Command::Vibrate(v));
    }
    if let Some(events) = wire.haptic {
        commands.push(Command::Haptic(
            events.into_iter().take(64).map(normalized).collect(),
        ));
    }
    if let Some(on) = wire.motion {
        commands.push(Command::Motion(on));
    }
    if let Some(allowed) = wire.orientation {
        commands.push(Command::Orientation(allowed));
    }
    if let Some(datagram) = wire.datagram {
        match &datagram {
            DatagramCommand::Open { port: 0, .. } => return None,
            DatagramCommand::Send { data, .. } if data.len() > 65_507 => return None,
            DatagramCommand::Open { .. }
            | DatagramCommand::Send { .. }
            | DatagramCommand::Close { .. } => {}
        }
        commands.push(Command::Datagram(datagram));
    }
    Some(commands)
}

fn command(value: &Command) -> String {
    // Every command is normalized before reaching the serializer.
    serde_json::to_string(value).unwrap_or_default()
}

pub fn vibrate(vibration: Vibration) -> String {
    command(&Command::Vibrate(vibration))
}

/// At most 64 events, as the app plays no more.
pub fn haptic(events: &[HapticEvent]) -> String {
    command(&Command::Haptic(
        events.iter().copied().take(64).map(normalized).collect(),
    ))
}

#[expect(
    clippy::fn_params_excessive_bools,
    reason = "the bool is the message: sent as is in the JSON the app reads"
)]
pub fn motion(on: bool) -> String {
    command(&Command::Motion(on))
}

pub fn orientation(allowed: Allowed) -> String {
    command(&Command::Orientation(allowed))
}

pub fn open(id: u32, port: u16) -> String {
    command(&Command::Datagram(DatagramCommand::Open { id, port }))
}

pub fn send(id: u32, data: &[u8]) -> String {
    command(&Command::Datagram(DatagramCommand::Send {
        id,
        data: data.to_vec(),
    }))
}

pub fn close(id: u32) -> String {
    command(&Command::Datagram(DatagramCommand::Close { id }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Motion,
    Orientation,
    Datagram,
}

impl EventKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Motion => "telecommande:motion",
            Self::Orientation => "telecommande:orientation",
            Self::Datagram => "telecommande:datagram",
        }
    }
}

pub fn bootstrap(info: &Info) -> Option<String> {
    let info = serde_json::to_string(info).ok()?;
    Some(format!("window.{NAME} = {info};"))
}

fn event(kind: EventKind, detail: &impl Serialize) -> Option<String> {
    let detail = serde_json::to_string(detail).ok()?;
    let name = kind.name();
    Some(format!(
        "window.dispatchEvent(new CustomEvent(\"{name}\", {{ detail: {detail} }}));"
    ))
}

pub fn motion_event(sample: &Motion) -> Option<String> {
    if !sample
        .gravity
        .iter()
        .chain(&sample.rotation)
        .chain(&sample.acceleration)
        .chain(&sample.attitude)
        .chain([&sample.time])
        .all(|v| v.is_finite())
    {
        return None;
    }
    event(EventKind::Motion, sample)
}

pub fn orientation_event(orientation: Orientation) -> Option<String> {
    let value = serde_json::to_string(&orientation).ok()?;
    Some(format!(
        "if (window.{NAME}) window.{NAME}.orientation = {value};{}",
        event(EventKind::Orientation, &orientation)?
    ))
}

pub fn datagram_event(id: u32, bytes: &[u8]) -> Option<String> {
    if bytes.len() > 65_507 {
        return None;
    }
    event(
        EventKind::Datagram,
        &Datagram {
            id,
            data: encode_base64(bytes),
        },
    )
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk.first().copied().unwrap_or_default();
        let b = chunk.get(1).copied().unwrap_or_default();
        let c = chunk.get(2).copied().unwrap_or_default();
        for digit in [a >> 2, ((a & 3) << 4) | (b >> 4)] {
            if let Some(&ch) = TABLE.get(usize::from(digit)) {
                out.push(char::from(ch));
            }
        }
        if chunk.len() > 1 {
            if let Some(&ch) = TABLE.get(usize::from(((b & 15) << 2) | (c >> 6))) {
                out.push(char::from(ch));
            }
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            if let Some(&ch) = TABLE.get(usize::from(c & 63)) {
                out.push(char::from(ch));
            }
        } else {
            out.push('=');
        }
    }
    out
}

pub fn read_info(json: &str) -> Option<Info> {
    read_object(json)
}

pub fn read_motion(json: &str) -> Option<Motion> {
    read_object(json)
}

fn read_object<T: serde::de::DeserializeOwned>(json: &str) -> Option<T> {
    // JSON's whitespace is ASCII, and the typed decoder checks all the rest.
    json.trim_start().starts_with('{').then_some(())?;
    serde_json::from_str(json).ok()
}

pub fn read_orientation(json: &str) -> Option<Orientation> {
    serde_json::from_str(json).ok()
}

/// The datagram's socket id and bytes.
pub fn read_datagram(json: &str) -> Option<(u32, Vec<u8>)> {
    let datagram: Datagram = read_object(json)?;
    let bytes = base64(&datagram.data)?;
    (bytes.len() <= 65_507).then_some((datagram.id, bytes))
}

/// Standard base64, as the app encodes; `None` if it is not.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "base64 decoding: each `c - b'X'` is in the arm where c >= b'X' and stays below 64; `bits` stays in 0..14, adding 6 and taking 8 only once it holds 8"
)]
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
    let mut bytes = Vec::with_capacity(digits.len().saturating_mul(3) / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &c in digits {
        buffer = (buffer << 6) | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
        }
    }
    (encode_base64(&bytes) == text).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parsed(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap_or_default()
    }

    #[test]
    fn messages_have_the_documented_shapes() {
        for (value, name) in [
            (Vibration::Press, "press"),
            (Vibration::Release, "release"),
            (Vibration::Success, "success"),
            (Vibration::Warning, "warning"),
        ] {
            assert_eq!(parsed(&vibrate(value)), json!({"vibrate": name}));
        }
        for (value, name) in [
            (Allowed::Portrait, "portrait"),
            (Allowed::Landscape, "landscape"),
            (Allowed::Any, "any"),
        ] {
            assert_eq!(parsed(&orientation(value)), json!({"orientation": name}));
        }
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
                player: "b17aad3e00000000000000000000beef".parse().unwrap(),
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
        assert!(read_info(r#"["b17aad3e00000000000000000000beef","Axel","portrait"]"#).is_none());
        assert!(read_motion("[[0,0,0],[0,0,0],[0,0,0],[0,0,0,1],0]").is_none());
        assert!(read_datagram(r#"[1,"YQ=="]"#).is_none());
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

    #[test]
    fn identities_and_datagrams_are_validated_before_becoming_types() {
        let valid = "0001abff09080706050403020100107f";
        assert_eq!(valid.parse::<PlayerId>(), valid.to_uppercase().parse());
        for invalid in [
            "",
            &valid[2..],
            &format!("{valid}00"),
            &format!("+{}", &valid[1..]),
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert!(invalid.parse::<PlayerId>().is_err());
            assert!(
                read_info(&format!(
                    r#"{{"player":"{invalid}","name":"x","orientation":"portrait"}}"#
                ))
                .is_none()
            );
        }
        for invalid in ["YQ", "YQ=", "YQ===", "YR==", "=YQ=", "YWJj=", "YQ==\n"] {
            assert!(
                base64(invalid).is_none(),
                "accepted malformed base64 {invalid:?}"
            );
        }
        for size in 0..1024 {
            let bytes = vec![173; size];
            assert_eq!(base64(&encode_base64(&bytes)), Some(bytes));
        }
    }

    #[test]
    fn native_commands_keep_legacy_batches_defaults_and_bounds() {
        let commands =
            read_commands(r#"{"motion":true,"haptic":[{}],"orientation":"any"}"#).unwrap();
        assert_eq!(
            commands,
            [
                Command::Haptic(vec![HapticEvent::tap(0.0, 1.0, 0.5)]),
                Command::Motion(true),
                Command::Orientation(Allowed::Any)
            ]
        );
        let events = [HapticEvent {
            time: f64::NAN,
            duration: f64::INFINITY,
            intensity: 4.0,
            sharpness: -1.0,
        }; 100];
        let decoded = read_commands(&haptic(&events)).unwrap();
        assert_eq!(
            decoded,
            [Command::Haptic(vec![
                HapticEvent {
                    time: 0.0,
                    duration: 0.0,
                    intensity: 1.0,
                    sharpness: 0.0
                };
                64
            ])]
        );
        for invalid in [
            "[null,null,true,null,null]",
            r#"{"haptic":[[0,0,1,0.5]]}"#,
            r#"{"motion":1}"#,
            r#"{"orientation":"sideways"}"#,
            r#"{"datagram":{"op":"send","id":-1,"data":[1]}}"#,
            r#"{"datagram":{"op":"send","id":1,"data":[256]}}"#,
            r#"{"datagram":{"op":"open","id":1,"port":0}}"#,
            r#"{"datagram":{"op":"open","id":1,"port":65536}}"#,
        ] {
            assert!(read_commands(invalid).is_none(), "accepted {invalid}");
        }
        assert!(read_commands(&send(1, &vec![0; 65_508])).is_none());
        let sample = Motion {
            gravity: [0.0; 3],
            rotation: [0.0; 3],
            acceleration: [0.0; 3],
            attitude: [0.0, 0.0, 0.0, 1.0],
            time: f64::NAN,
        };
        assert!(motion_event(&sample).is_none());
    }
}
