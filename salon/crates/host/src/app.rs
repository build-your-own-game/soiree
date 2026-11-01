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

use crate::admission::DeadlineReader;
use crate::clock::Clock;

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
    run_with_handshake(stream, page, HANDSHAKE);
}

fn run_with_handshake(stream: TcpStream, page: &str, handshake: Duration) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let mut socket = WebSocket::from_raw_socket(
        DeadlineReader::new(stream, handshake),
        Role::Server,
        Some(config),
    );
    let opened = Clock::instant();
    let mut greeted = false;
    loop {
        if !greeted && Clock::instant().duration_since(opened) >= handshake {
            break;
        }
        if greeted {
            socket.get_mut().renew(Duration::from_secs(30));
        }
        match socket.read() {
            Ok(Message::Binary(frame)) => {
                if let Some(answer) = answer(&frame, page) {
                    greeted = true;
                    socket.get_mut().renew_write(Duration::from_secs(10));
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
                if !greeted {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    // The page is gone or done: nothing to do if the goodbye fails.
    socket.get_mut().renew_write(Duration::from_secs(10));
    let _: tungstenite::Result<()> = socket.close(None);
    let _: tungstenite::Result<()> = socket.flush();
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

    #[test]
    fn malformed_complete_frames_do_not_keep_an_ungreeted_app_alive() {
        use std::io::Read;
        use std::net::{Shutdown, TcpListener};
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let task =
            thread::spawn(move || run_with_handshake(server, "/play", Duration::from_millis(80)));
        let writer_stream = client.try_clone().unwrap();
        let writer = thread::spawn(move || {
            let mut socket = WebSocket::from_raw_socket(writer_stream, Role::Client, None);
            for _ in 0..50 {
                if socket.send(Message::binary(b"garbage".to_vec())).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut close = [0; 2];
        let result = client.read_exact(&mut close);
        let _: std::io::Result<()> = client.shutdown(Shutdown::Both);
        writer.join().unwrap();
        task.join().unwrap();
        result.unwrap();
        assert_eq!(
            close[0], 0x88,
            "ungreeted peer was not sent a WebSocket close"
        );
    }

    #[test]
    fn a_greeted_app_can_stay_idle_past_the_handshake_deadline() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let task =
            thread::spawn(move || run_with_handshake(server, "/play", Duration::from_millis(80)));
        let mut socket = WebSocket::from_raw_socket(client, Role::Client, None);
        let hello = encode(&PhoneMessage::Hello {
            version: VERSION,
            player: [1; 16],
            name: "Ada".to_owned(),
        });
        socket.send(Message::binary(hello.clone())).unwrap();
        assert!(matches!(socket.read().unwrap(), Message::Binary(_)));
        thread::sleep(Duration::from_millis(120));
        socket.send(Message::binary(hello)).unwrap();
        assert!(matches!(socket.read().unwrap(), Message::Binary(_)));
        socket.close(None).unwrap();
        task.join().unwrap();
    }
}
