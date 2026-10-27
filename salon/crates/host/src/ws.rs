//! One WebSocket link: binary frames carry salon's packets, both ways.
//!
//! Reading and writing run on two threads, so a packet leaves as soon as the
//! room hands it over, instead of waiting for a read to time out: on the TV
//! link, that wait was jitter on every frame. Both write through one lock,
//! whole buffers at a time, so frames never interleave.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use salon_core::{Caps, LinkId};
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::Frame;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::frame::coding::{Data, OpCode};
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use crate::room::{Net, OUTBOX, Outlet, WsOut};

/// Bigger frames close the link (salon's packets are at most 16 KiB here).
const MAX_FRAME: usize = 64 * 1024;

/// The socket as tungstenite's reader sees it: reads its own handle, writes
/// (pongs, the closing handshake) through the shared lock.
struct Reading {
    read: TcpStream,
    write: Arc<Mutex<TcpStream>>,
}

impl Read for Reading {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read.read(buffer)
    }
}

impl Write for Reading {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut stream = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        stream.write_all(buffer)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .flush()
    }
}

pub fn run(stream: TcpStream, link: LinkId, net: &SyncSender<Net>) {
    if stream.set_nodelay(true).is_err() {
        return;
    }
    // Only a link from this very computer may be the TV (salon-core, `Caps`).
    let local = stream
        .peer_addr()
        .is_ok_and(|peer| peer.ip().to_canonical().is_loopback());
    let Ok(write) = stream.try_clone() else {
        return;
    };
    let write = Arc::new(Mutex::new(write));
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let reading = Reading {
        read: stream,
        write: Arc::clone(&write),
    };
    let mut socket = WebSocket::from_raw_socket(reading, Role::Server, Some(config));
    let (outbox, outgoing) = sync_channel(OUTBOX);
    let opened = Net::Opened {
        link,
        caps: if local {
            Caps::STREAM.local()
        } else {
            Caps::STREAM
        },
        outlet: Outlet::Ws(outbox),
    };
    if net.send(opened).is_err() {
        return;
    }
    let writer = Arc::clone(&write);
    thread::spawn(move || send(&outgoing, &writer));
    loop {
        match socket.read() {
            Ok(Message::Binary(bytes)) => {
                if net
                    .send(Net::Received {
                        link,
                        bytes: bytes.to_vec(),
                    })
                    .is_err()
                {
                    break;
                }
            }
            Err(tungstenite::Error::Capacity(_)) => {
                // Too big (E2): say why before closing, as RFC 6455 asks (1009).
                let close = Frame::close(Some(CloseFrame {
                    code: CloseCode::Size,
                    reason: "message too big".into(),
                }));
                let mut buffer = Vec::new();
                if close.format(&mut buffer).is_ok() {
                    let _ = write
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .write_all(&buffer);
                }
                break;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
    // Wakes the writer if it is blocked, and tells the room.
    let _ = write
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .shutdown(Shutdown::Both);
    let _ = net.send(Net::Closed { link });
}

/// Writes what the room sends, one whole frame at a time, until it says
/// close or forgets the link.
fn send(outgoing: &Receiver<WsOut>, write: &Mutex<TcpStream>) {
    for out in outgoing {
        let (frame, last) = match out {
            WsOut::Packet(bytes) => (
                Frame::message(bytes, OpCode::Data(Data::Binary), true),
                false,
            ),
            WsOut::Close => (Frame::close(None), true),
        };
        let mut buffer = Vec::new();
        if frame.format(&mut buffer).is_err() {
            break;
        }
        let mut stream = write.lock().unwrap_or_else(PoisonError::into_inner);
        if stream.write_all(&buffer).is_err() || last {
            let _ = stream.shutdown(Shutdown::Both);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::sync_channel;
    use std::thread;
    use std::time::Duration;

    use tungstenite::protocol::Role;
    use tungstenite::{Message, WebSocket};

    use super::*;

    /// E2: a frame over the limit closes that link, and nothing of it reaches
    /// the room.
    #[test]
    fn an_oversized_frame_closes_its_link() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (net, inbox) = sync_channel(64);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            run(stream, LinkId(1), &net);
        });
        let mut client =
            WebSocket::from_raw_socket(TcpStream::connect(address).unwrap(), Role::Client, None);
        let Net::Opened {
            link: LinkId(1),
            caps,
            outlet: Outlet::Ws(outbox),
        } = inbox.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("the link did not open");
        };
        assert!(caps.local);
        // What the room sends leaves at once, without waiting for a read.
        let sent = crate::clock::Clock::instant();
        outbox.send(WsOut::Packet(vec![1, 2, 3])).unwrap();
        assert_eq!(client.read().unwrap(), Message::binary(vec![1_u8, 2, 3]));
        assert!(
            sent.elapsed() < Duration::from_millis(50),
            "{:?}",
            sent.elapsed()
        );
        client.send(Message::binary(vec![7_u8; 10])).unwrap();
        assert!(
            matches!(inbox.recv_timeout(Duration::from_secs(5)).unwrap(), Net::Received { bytes, .. } if bytes.len() == 10)
        );
        let _ = client.send(Message::binary(vec![0_u8; MAX_FRAME + 1]));
        let closed = inbox.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(closed, Net::Closed { link: LinkId(1) }));
        // The client is told why: 1009, message too big.
        match client.read() {
            Ok(Message::Close(Some(frame))) => assert_eq!(frame.code, CloseCode::Size),
            other => panic!("expected a close frame with 1009, got {other:?}"),
        }
        server.join().unwrap();
    }
}
