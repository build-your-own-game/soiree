//! A game that does what a test says: serves `/remote` and pages, cuts,
//! freezes, stays silent, and records all it hears.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use remote_protocol::{GameMessage, PhoneMessage, Remote, decode, encode};
use tungstenite::{Message, WebSocket};

/// What the game answers on a path.
#[derive(Clone)]
pub enum Page {
    Html(String),
    Redirect(String),
}

/// A frame from the phone, and which socket brought it (1 is the first).
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub link: usize,
    pub message: PhoneMessage,
}

#[derive(Default)]
pub struct State {
    /// What each `Hello` gets.
    pub show: Option<Remote>,
    pub heard: Vec<Heard>,
    /// Requests for pages, path and query.
    pub requests: Vec<String>,
    /// TCP connections of any kind.
    pub accepted: usize,
    /// Sockets opened on `/remote`.
    pub links: usize,
    pub routes: BTreeMap<String, Page>,
    /// Close the WebSocket now.
    pub cut: bool,
    /// Read nothing and answer nothing, sockets open: what a frozen host does.
    pub frozen: bool,
    /// Accept TCP and never answer: an address gone mute.
    pub silent: bool,
    /// Close this many `/remote` requests without opening them.
    pub refuse: usize,
    /// Close this many page requests without an answer.
    pub broken_pages: usize,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

/// The game a test joins. It serves until the test binary exits.
#[derive(Clone)]
pub struct FakeHost {
    shared: Arc<Shared>,
    pub port: u16,
}

impl FakeHost {
    pub fn start(show: Option<Remote>) -> FakeHost {
        let listener = TcpListener::bind("0.0.0.0:0").expect("a port");
        let port = listener.local_addr().expect("a port").port();
        let shared = Arc::new(Shared::default());
        shared.state.lock().expect("state").show = show;
        let accepting = Arc::clone(&shared);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = Arc::clone(&accepting);
                thread::spawn(move || serve(stream, &shared));
            }
        });
        FakeHost { shared, port }
    }

    /// The address a QR code would hold, on loopback.
    pub fn page(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// Reads or changes the state, and wakes whoever waits on it.
    pub fn with<T>(&self, change: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.shared.state.lock().expect("state");
        let result = change(&mut state);
        self.shared.changed.notify_all();
        result
    }

    /// Until `done` holds; false after `limit`.
    pub fn wait(&self, limit: Duration, mut done: impl FnMut(&State) -> bool) -> bool {
        let start = Instant::now();
        let mut state = self.shared.state.lock().expect("state");
        loop {
            if done(&state) {
                return true;
            }
            let left = limit.saturating_sub(start.elapsed());
            if left.is_zero() {
                return false;
            }
            state = self
                .shared
                .changed
                .wait_timeout(state, left.min(Duration::from_millis(50)))
                .expect("state")
                .0;
        }
    }

    pub fn messages(&self, link: usize) -> Vec<PhoneMessage> {
        self.with(|state| {
            state
                .heard
                .iter()
                .filter(|heard| heard.link == link)
                .map(|heard| heard.message.clone())
                .collect()
        })
    }
}

/// The first line of the request, without consuming it.
fn request_line(stream: &TcpStream) -> Option<String> {
    let mut buffer = [0; 2048];
    for _ in 0..500 {
        let peeked = stream.peek(&mut buffer).ok()?;
        if peeked == 0 {
            return None;
        }
        if let Some(end) = buffer.iter().take(peeked).position(|byte| *byte == b'\r') {
            return Some(String::from_utf8_lossy(&buffer[..end]).into_owned());
        }
        thread::sleep(Duration::from_millis(2));
    }
    None
}

fn serve(stream: TcpStream, shared: &Shared) {
    {
        let mut state = shared.state.lock().expect("state");
        state.accepted += 1;
        shared.changed.notify_all();
    }
    while {
        let state = shared.state.lock().expect("state");
        state.silent || state.frozen
    } {
        thread::sleep(Duration::from_millis(20));
    }
    let Some(line) = request_line(&stream) else {
        return;
    };
    let target = line.split(' ').nth(1).unwrap_or_default().to_owned();
    if target.starts_with("/remote") {
        let refused = {
            let mut state = shared.state.lock().expect("state");
            let refused = state.refuse > 0;
            state.refuse = state.refuse.saturating_sub(1);
            refused
        };
        if refused {
            let _: std::io::Result<()> = stream.shutdown(Shutdown::Both);
        } else {
            game(stream, shared);
        }
    } else {
        page(stream, &target, shared);
    }
}

fn page(mut stream: TcpStream, target: &str, shared: &Shared) {
    let answer = {
        let mut state = shared.state.lock().expect("state");
        state.requests.push(target.to_owned());
        shared.changed.notify_all();
        if state.broken_pages > 0 {
            state.broken_pages -= 1;
            None
        } else {
            let path = target.split('?').next().unwrap_or_default();
            Some(state.routes.get(path).cloned())
        }
    };
    let Some(answer) = answer else {
        let _: std::io::Result<()> = stream.shutdown(Shutdown::Both);
        return;
    };
    let reader = BufReader::new(stream.try_clone().expect("a stream"));
    for line in reader.lines() {
        if line.map_or(true, |line| line.is_empty()) {
            break;
        }
    }
    let response = match answer {
        Some(Page::Html(body)) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
        Some(Page::Redirect(to)) => format!(
            "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        None => {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
        }
    };
    let _: std::io::Result<()> = stream.write_all(response.as_bytes());
}

fn game(stream: TcpStream, shared: &Shared) {
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return;
    };
    let link = {
        let mut state = shared.state.lock().expect("state");
        state.links += 1;
        state.cut = false;
        shared.changed.notify_all();
        state.links
    };
    socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(10)))
        .expect("a stream");
    loop {
        let (cut, frozen) = {
            let state = shared.state.lock().expect("state");
            (state.cut, state.frozen)
        };
        if cut {
            let _: std::io::Result<()> = socket.get_ref().shutdown(Shutdown::Both);
            return;
        }
        if frozen {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        if !read_one(&mut socket, link, shared) {
            return;
        }
    }
}

/// One frame, if one came. False when the socket is over.
fn read_one(socket: &mut WebSocket<TcpStream>, link: usize, shared: &Shared) -> bool {
    match socket.read() {
        Ok(Message::Binary(frame)) => {
            let Ok(message) = decode::<PhoneMessage>(&frame) else {
                return true;
            };
            let hello = matches!(message, PhoneMessage::Hello { .. });
            let show = {
                let mut state = shared.state.lock().expect("state");
                state.heard.push(Heard { link, message });
                shared.changed.notify_all();
                state.show.clone()
            };
            if hello && let Some(remote) = show {
                let frame = encode(&GameMessage::Show(remote));
                return socket.send(Message::binary(frame)).is_ok();
            }
            true
        }
        Ok(Message::Close(_)) => false,
        Ok(_) => true,
        Err(tungstenite::Error::Io(error))
            if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
        {
            true
        }
        Err(_) => false,
    }
}
