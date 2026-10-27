//! The iPhone app's door, `/remote` (`remote/CONTRAT.md`): the app scans the
//! TV's QR code, connects here and says `Hello`; it is told to show the
//! game's phone page ([`crate::Options::app_page`]). From then on the page,
//! inside the app, talks to the game through salon like any phone, with the
//! app's sensors on top. Another version of the app gets `Refuse`, with a
//! reason it shows.
//!
//! The socket stays open, since the app says `Hello` again after each
//! reconnection; anything else it sends is ignored. A socket that says
//! nothing for [`HANDSHAKE`] is closed.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::time::Duration;

use remote_protocol::{GameMessage, PhoneMessage, Remote, VERSION, decode, encode};
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use crate::clock::Clock;

const POLL: Duration = Duration::from_millis(250);
const HANDSHAKE: Duration = Duration::from_secs(10);
const MAX_FRAME: usize = 16 * 1024;

/// What to answer to one frame from the app, if anything.
pub fn answer(frame: &[u8], page: &str) -> Option<GameMessage> {
    match decode::<PhoneMessage>(frame).ok()? {
        PhoneMessage::Hello { version, .. } if version == VERSION => {
            Some(GameMessage::Show(Remote::Web {
                url: page.to_owned(),
            }))
        }
        PhoneMessage::Hello { version, .. } => Some(GameMessage::Refuse {
            reason: format!(
                "This app speaks version {version} of the remote protocol; this game speaks \
                 version {VERSION}. Update the app, or open the game in the browser."
            ),
        }),
        PhoneMessage::Input(_) | PhoneMessage::Draw(_) => None,
    }
}

pub fn run(stream: TcpStream, page: &str) {
    if stream.set_read_timeout(Some(POLL)).is_err() {
        return;
    }
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let mut socket = WebSocket::from_raw_socket(stream, Role::Server, Some(config));
    let opened = Clock::instant();
    let mut greeted = false;
    loop {
        match socket.read() {
            Ok(Message::Binary(frame)) => {
                if let Some(answer) = answer(&frame, page) {
                    greeted = true;
                    if socket.send(Message::binary(encode(&answer))).is_err() {
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                if !greeted && Clock::instant().duration_since(opened) > HANDSHAKE {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = socket.close(None);
    let _ = socket.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_is_sent_to_the_phone_page() {
        let hello = encode(&PhoneMessage::Hello {
            version: VERSION,
            player: [1; 16],
            name: "Ada".to_owned(),
        });
        assert_eq!(
            answer(&hello, "/play"),
            Some(GameMessage::Show(Remote::Web {
                url: "/play".to_owned()
            }))
        );
        let old = encode(&PhoneMessage::Hello {
            version: VERSION + 1,
            player: [1; 16],
            name: "Ada".to_owned(),
        });
        assert!(matches!(
            answer(&old, "/play"),
            Some(GameMessage::Refuse { .. })
        ));
        assert_eq!(answer(b"garbage", "/play"), None);
    }
}
