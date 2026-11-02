//! The iPhone app's door, `/remote` (`remote/CONTRAT.md`): the app scans the
//! TV's QR code, connects here and says `Hello`. The room answers with what it
//! must show: the game's phone page ([`crate::Options::app_page`]), which then
//! talks to the game through salon like any phone with the app's sensors on
//! top, or the native remote the game chose ([`crate::Game::remote`]). Another
//! version of the app gets `Refuse`, with a reason it shows, then the socket
//! closes.
//!
//! This thread only reads: the app's states and strokes go to the room, in
//! order, and what the room says leaves through the same writer as a
//! WebSocket link's packets. The socket stays open, since the app says `Hello`
//! again after each reconnection and its page may stay quiet for as long as it
//! likes. A socket that says nothing for [`HANDSHAKE`] is closed.

use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use remote_protocol::web::PlayerId;
use remote_protocol::{GameMessage, PhoneMessage, VERSION, decode, encode};
use salon_core::LinkId;
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use crate::admission::DeadlineReader;
use crate::clock::Clock;
use crate::driver::Input;
use crate::room::{Net, OUTBOX, Outlet, inbox};
use crate::ws::{self, Reading};

const HANDSHAKE: Duration = Duration::from_secs(10);
const MAX_FRAME: usize = 16 * 1024;

/// What one frame from the app means.
enum Heard {
    /// For the room.
    Room(Input),
    /// Answered here: the room has nothing to say to another protocol.
    Refused(GameMessage),
    /// Unreadable. It does not keep an unknown socket alive.
    Nothing,
}

fn heard(frame: &[u8], link: LinkId) -> Heard {
    match decode::<PhoneMessage>(frame) {
        Ok(PhoneMessage::Hello {
            version,
            player,
            name,
        }) if version == VERSION => Heard::Room(Input::AppHello {
            link,
            player: PlayerId::new(player),
            name,
        }),
        Ok(PhoneMessage::Hello { version, .. }) => Heard::Refused(GameMessage::Refuse {
            reason: format!(
                "This app speaks version {version} of the remote protocol; this game speaks \
                 version {VERSION}. Update the app, or open the game in the browser."
            ),
        }),
        Ok(PhoneMessage::Input(state)) => Heard::Room(Input::AppInput { link, state }),
        Ok(PhoneMessage::Draw(event)) => Heard::Room(Input::AppDraw { link, event }),
        Err(_) => Heard::Nothing,
    }
}

pub fn run(stream: TcpStream, link: LinkId, net: &inbox::Sender<Net>) {
    run_with_handshake(stream, link, net, HANDSHAKE);
}

fn run_with_handshake(
    stream: TcpStream,
    link: LinkId,
    net: &inbox::Sender<Net>,
    handshake: Duration,
) {
    if stream.set_nodelay(true).is_err()
        || stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .is_err()
    {
        return;
    }
    let (Ok(write), Ok(shutdown)) = (stream.try_clone(), stream.try_clone()) else {
        return;
    };
    let write = Arc::new(Mutex::new(write));
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let reading = Reading {
        read: DeadlineReader::new(stream, handshake),
        write: Arc::clone(&write),
    };
    let mut socket = WebSocket::from_raw_socket(reading, Role::Server, Some(config));
    let (outbox, outgoing) = sync_channel(OUTBOX);
    let opened = Net::AppOpened {
        link,
        outlet: Outlet::Ws(outbox),
    };
    if net.send(opened).is_err() {
        return;
    }
    let writer = Arc::clone(&write);
    let stopped = Arc::new(AtomicBool::new(false));
    let writer_stopped = Arc::clone(&stopped);
    let writer_thread = thread::spawn(move || ws::send(&outgoing, &writer, &writer_stopped));
    let opened = Clock::instant();
    let mut greeted = false;
    loop {
        if !greeted && Clock::instant().duration_since(opened) >= handshake {
            break;
        }
        if greeted {
            socket.get_mut().read.renew(Duration::from_secs(30));
        }
        match socket.read() {
            Ok(Message::Binary(frame)) => match heard(&frame, link) {
                Heard::Room(input) => {
                    greeted |= matches!(input, Input::AppHello { .. });
                    if net.send(Net::App(input)).is_err() {
                        break;
                    }
                }
                Heard::Refused(answer) => {
                    // From here, so that the close below comes behind it. The
                    // app has nothing more to say: it would keep this thread
                    // and its permit for as long as it stayed.
                    let _: tungstenite::Result<()> = socket.send(Message::binary(encode(&answer)));
                    break;
                }
                Heard::Nothing => {}
            },
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
    let _: tungstenite::Result<()> = socket.close(None);
    let _: tungstenite::Result<()> = socket.flush();
    // Wakes the writer if it is blocked, and tells the room.
    stopped.store(true, Ordering::Relaxed);
    let _: std::io::Result<()> = shutdown.shutdown(Shutdown::Both);
    // The room is gone only when the host stops.
    let _: std::io::Result<()> = net.send(Net::Closed { link });
    let _: std::thread::Result<()> = writer_thread.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    use remote_protocol::Remote;

    #[test]
    fn another_version_is_refused_here_and_garbage_is_nothing() {
        let hello = |version| {
            encode(&PhoneMessage::Hello {
                version,
                player: [1; 16],
                name: "Ada".to_owned(),
            })
        };
        let link = LinkId(1);
        assert!(matches!(
            heard(&hello(VERSION), link),
            Heard::Room(Input::AppHello { name, .. }) if name == "Ada"
        ));
        assert!(matches!(
            heard(&hello(VERSION + 1), link),
            Heard::Refused(GameMessage::Refuse { .. })
        ));
        assert!(matches!(heard(b"garbage", link), Heard::Nothing));
    }

    #[test]
    fn malformed_complete_frames_do_not_keep_an_ungreeted_app_alive() {
        use std::io::Read;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let (net, _inbox) = inbox::channel(16).unwrap();
        let task = thread::spawn(move || {
            run_with_handshake(server, LinkId(1), &net, Duration::from_millis(80));
        });
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

    /// The contract refuses another version with a reason the app shows. The
    /// connection ends there: said nothing more, it kept its thread and its
    /// permit for as long as the app stayed.
    #[test]
    fn a_refused_app_reads_why_and_gives_its_permit_back() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let (net, inbox) = inbox::channel(16).unwrap();
        let task = thread::spawn(move || {
            run_with_handshake(server, LinkId(1), &net, Duration::from_millis(50));
        });
        let mut socket = WebSocket::from_raw_socket(client, Role::Client, None);
        let hello = encode(&PhoneMessage::Hello {
            version: VERSION + 1,
            player: [1; 16],
            name: String::new(),
        });
        socket.send(Message::binary(hello)).unwrap();
        let Message::Binary(answer) = socket.read().unwrap() else {
            panic!("the reason, first");
        };
        assert!(matches!(
            decode::<GameMessage>(&answer),
            Ok(GameMessage::Refuse { .. })
        ));
        assert!(matches!(socket.read(), Ok(Message::Close(_)) | Err(_)));
        let start = Clock::instant();
        while !task.is_finished() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the thread and its permit stay taken"
            );
            thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(
            inbox.recv_timeout(Duration::from_secs(5)),
            Ok(Net::AppOpened { .. })
        ));
        assert!(matches!(
            inbox.recv_timeout(Duration::from_secs(5)),
            Ok(Net::Closed { link: LinkId(1) })
        ));
    }

    #[test]
    fn a_greeted_app_can_stay_idle_past_the_handshake_deadline() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let (net, inbox) = inbox::channel(16).unwrap();
        let task = thread::spawn(move || {
            run_with_handshake(server, LinkId(1), &net, Duration::from_millis(80));
        });
        // The room: one answer to each hello, until the link closes.
        let room = thread::spawn(move || {
            let mut outlet = None;
            loop {
                match inbox.recv_timeout(Duration::from_secs(5)).unwrap() {
                    Net::AppOpened { outlet: opened, .. } => outlet = Some(opened),
                    Net::App(Input::AppHello { link, .. }) => {
                        let shown = GameMessage::Show(Remote::Web {
                            url: "/play".to_owned(),
                        });
                        outlet.as_ref().unwrap().send(link, encode(&shown));
                    }
                    Net::Closed { .. } => return,
                    Net::App(_)
                    | Net::Diagnostic(_)
                    | Net::Opened { .. }
                    | Net::Received { .. } => panic!("nothing else comes from this app"),
                }
            }
        });
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
        room.join().unwrap();
    }
}
