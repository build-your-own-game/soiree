//! Swift sees generated enums and records; JSON is owned by the same Rust
//! contract the WASM page uses. Exhaustive conversions keep this FFI face
//! aligned when the protocol grows.

use remote_protocol::web as wire;

use crate::Haptic;

#[derive(uniffi::Enum)]
pub enum WebOrientation {
    Portrait,
    LandscapeLeft,
    LandscapeRight,
}

impl From<WebOrientation> for wire::Orientation {
    fn from(value: WebOrientation) -> Self {
        match value {
            WebOrientation::Portrait => Self::Portrait,
            WebOrientation::LandscapeLeft => Self::LandscapeLeft,
            WebOrientation::LandscapeRight => Self::LandscapeRight,
        }
    }
}

impl From<wire::Orientation> for WebOrientation {
    fn from(value: wire::Orientation) -> Self {
        match value {
            wire::Orientation::Portrait => Self::Portrait,
            wire::Orientation::LandscapeLeft => Self::LandscapeLeft,
            wire::Orientation::LandscapeRight => Self::LandscapeRight,
        }
    }
}

#[derive(uniffi::Enum)]
pub enum WebAllowed {
    Landscape,
    Portrait,
    Any,
}

impl From<WebAllowed> for wire::Allowed {
    fn from(value: WebAllowed) -> Self {
        match value {
            WebAllowed::Landscape => Self::Landscape,
            WebAllowed::Portrait => Self::Portrait,
            WebAllowed::Any => Self::Any,
        }
    }
}

impl From<wire::Allowed> for WebAllowed {
    fn from(value: wire::Allowed) -> Self {
        match value {
            wire::Allowed::Landscape => Self::Landscape,
            wire::Allowed::Portrait => Self::Portrait,
            wire::Allowed::Any => Self::Any,
        }
    }
}

#[derive(uniffi::Record)]
pub struct WebHapticEvent {
    pub time: f64,
    pub duration: f64,
    pub intensity: f64,
    pub sharpness: f64,
}

#[derive(uniffi::Record)]
pub struct WebVector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<WebVector3> for [f64; 3] {
    fn from(v: WebVector3) -> Self {
        [v.x, v.y, v.z]
    }
}

#[derive(uniffi::Record)]
pub struct WebQuaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[derive(uniffi::Record)]
pub struct WebMotion {
    pub gravity: WebVector3,
    pub rotation: WebVector3,
    pub acceleration: WebVector3,
    pub attitude: WebQuaternion,
    pub time: f64,
}

#[derive(uniffi::Enum)]
pub enum WebCommand {
    Vibrate { haptic: Haptic },
    Haptic { events: Vec<WebHapticEvent> },
    Motion { enabled: bool },
    Orientation { allowed: WebAllowed },
    OpenDatagram { id: u32, port: u16 },
    SendDatagram { id: u32, data: Vec<u8> },
    CloseDatagram { id: u32 },
}

impl From<wire::Command> for WebCommand {
    fn from(value: wire::Command) -> Self {
        match value {
            wire::Command::Vibrate(v) => Self::Vibrate {
                haptic: match v {
                    wire::Vibration::Press => Haptic::Press,
                    wire::Vibration::Release => Haptic::Release,
                    wire::Vibration::Success => Haptic::Success,
                    wire::Vibration::Warning => Haptic::Warning,
                },
            },
            wire::Command::Haptic(events) => Self::Haptic {
                events: events
                    .into_iter()
                    .map(|v| {
                        let wire::HapticEvent {
                            time,
                            duration,
                            intensity,
                            sharpness,
                        } = v;
                        WebHapticEvent {
                            time,
                            duration,
                            intensity,
                            sharpness,
                        }
                    })
                    .collect(),
            },
            wire::Command::Motion(enabled) => Self::Motion { enabled },
            wire::Command::Orientation(v) => Self::Orientation { allowed: v.into() },
            wire::Command::Datagram(v) => match v {
                wire::DatagramCommand::Open { id, port } => Self::OpenDatagram { id, port },
                wire::DatagramCommand::Send { id, data } => Self::SendDatagram { id, data },
                wire::DatagramCommand::Close { id } => Self::CloseDatagram { id },
            },
        }
    }
}

#[uniffi::export]
pub fn web_handler_name() -> String {
    wire::NAME.into()
}

#[uniffi::export]
#[expect(
    clippy::needless_pass_by_value,
    reason = "UniFFI owns strings crossing the Swift boundary"
)]
pub fn web_commands(json: String) -> Vec<WebCommand> {
    wire::read_commands(&json)
        .unwrap_or_default()
        .into_iter()
        .map(WebCommand::from)
        .collect()
}

#[uniffi::export]
pub fn web_bootstrap(player: Vec<u8>, name: String, orientation: WebOrientation) -> Option<String> {
    let player: [u8; 16] = player.try_into().ok()?;
    wire::bootstrap(&wire::Info {
        player: wire::PlayerId::new(player),
        name,
        orientation: orientation.into(),
    })
}

#[uniffi::export]
pub fn web_orientation(orientation: WebOrientation) -> Option<String> {
    wire::orientation_event(orientation.into())
}

#[uniffi::export]
pub fn web_motion(sample: WebMotion) -> Option<String> {
    wire::motion_event(&wire::Motion {
        gravity: sample.gravity.into(),
        rotation: sample.rotation.into(),
        acceleration: sample.acceleration.into(),
        attitude: [
            sample.attitude.x,
            sample.attitude.y,
            sample.attitude.z,
            sample.attitude.w,
        ],
        time: sample.time,
    })
}

#[uniffi::export]
#[expect(
    clippy::needless_pass_by_value,
    reason = "UniFFI owns byte buffers crossing the Swift boundary"
)]
pub fn web_datagram(id: u32, data: Vec<u8>) -> Option<String> {
    wire::datagram_event(id, &data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_messages_are_decoded_by_the_native_app_contract() {
        for (json, check) in [
            (wire::vibrate(wire::Vibration::Press), 0),
            (wire::haptic(&[wire::HapticEvent::tap(0.1, 0.5, 0.8)]), 1),
            (wire::motion(true), 2),
            (wire::orientation(wire::Allowed::Portrait), 3),
            (wire::open(9, 8095), 4),
            (wire::send(9, &[0, 255, 34]), 5),
            (wire::close(9), 6),
        ] {
            let commands = web_commands(json);
            assert_eq!(commands.len(), 1);
            let actual = match commands.first().unwrap() {
                WebCommand::Vibrate {
                    haptic: Haptic::Press,
                } => 0,
                WebCommand::Haptic { events } => {
                    assert_eq!(events[0].intensity, 0.5);
                    1
                }
                WebCommand::Motion { enabled } => {
                    assert!(*enabled);
                    2
                }
                WebCommand::Orientation {
                    allowed: WebAllowed::Portrait,
                } => 3,
                WebCommand::Vibrate { .. } | WebCommand::Orientation { .. } => 7,
                WebCommand::OpenDatagram { id, port } => {
                    assert_eq!((*id, *port), (9, 8095));
                    4
                }
                WebCommand::SendDatagram { data, .. } => {
                    assert_eq!(data, &[0, 255, 34]);
                    5
                }
                WebCommand::CloseDatagram { id } => {
                    assert_eq!(*id, 9);
                    6
                }
            };
            assert_eq!(actual, check);
        }
    }

    #[test]
    fn native_identity_and_script_escaping_share_the_page_decoder() {
        assert!(web_bootstrap(vec![0; 15], "x".into(), WebOrientation::Portrait).is_none());
        let name = "A\"\\\n</script>é";
        let script =
            web_bootstrap(vec![17; 16], name.into(), WebOrientation::LandscapeRight).unwrap();
        let json = script
            .strip_prefix("window.telecommande = ")
            .unwrap()
            .strip_suffix(';')
            .unwrap();
        let info = wire::read_info(json).unwrap();
        assert_eq!(info.player.bytes(), [17; 16]);
        assert_eq!(info.name, name);
    }
}
