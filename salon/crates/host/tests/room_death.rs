//! A dead room must not leave a server that still answers: phones and the TV
//! would retry against it for ever. A real game panics in a real server, in a
//! child process, so that its exit code and its log can be read.

#![expect(
    clippy::unwrap_used,
    reason = "test: its helpers are outside the #[test] functions Clippy recognizes"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use salon_core::{DeviceEvent, Host, Ms, route};
use salon_host::{Clock, Game, HttpHandler, Options, ROOM_DIED, Request, Response};

const PORT: &str = "SALON_TEST_ROOM_DEATH_PORT";
const BOOM: &str = "/boom";
const WHY: &str = "this game breaks on purpose";

/// Asked by `POST /boom`, breaks at its next tick, on the room's thread.
struct Doomed {
    asked: Arc<AtomicBool>,
}

impl Game for Doomed {
    type Up = ();
    type Down = ();

    fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}

    fn tick(&mut self, _: Ms, _: &mut Host<(), ()>) {
        assert!(!self.asked.load(Ordering::Relaxed), "{WHY}");
    }

    fn http(&self) -> Option<HttpHandler> {
        let asked = Arc::clone(&self.asked);
        Some(Arc::new(move |request: &Request| {
            (request.path == BOOM).then(|| {
                asked.store(true, Ordering::Relaxed);
                Response::json(Vec::new())
            })
        }))
    }
}

/// The server of [`a_dead_room_ends_the_process`], in its child process.
#[test]
#[ignore = "only a server: a_dead_room_ends_the_process runs it in a child process"]
fn serve_a_doomed_game() {
    let Ok(port) = std::env::var(PORT) else {
        return;
    };
    let mut options = Options::new(
        port.parse().unwrap(),
        std::env::temp_dir(),
        1,
        "doomed ready",
    );
    options.tick = Duration::from_millis(5);
    options.data = Some(std::env::temp_dir().join(format!("salon-room-death-{port}")));
    let asked = Arc::new(AtomicBool::new(false));
    salon_host::run(options, Doomed { asked }).unwrap();
}

/// Kills a child left behind by a failed assertion.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _: std::io::Result<()> = self.0.kill();
        let _: std::io::Result<_> = self.0.wait();
    }
}

/// Free on TCP and on UDP, which salon opens with the same number.
fn free_port() -> u16 {
    loop {
        let tcp = std::net::TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            return port;
        }
    }
}

/// Starts the server on `port`, and waits for it.
fn start(port: u16, logging: Option<&str>) -> (Server, impl Sized) {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "serve_a_doomed_game", "--ignored", "--nocapture"])
        .env(PORT, port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(logging) = logging {
        child.env("SALON_LOG", logging);
    }
    let mut server = Server(child.spawn().unwrap());
    let mut stdout = BufReader::new(server.0.stdout.take().unwrap()).lines();
    let ready = stdout
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.contains("doomed ready"));
    assert!(ready, "the server did not start");
    (server, stdout)
}

/// A WebSocket to the server, with a read that gives up after 5 s.
fn socket(port: u16) -> tungstenite::WebSocket<TcpStream> {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let address = format!("ws://127.0.0.1:{port}{}", route::SOCKET);
    tungstenite::client(address.as_str(), stream).unwrap().0
}

/// The peer closed. A read that timed out says nothing of that, and it is an
/// error too.
fn is_closed(read: tungstenite::Result<tungstenite::Message>) -> bool {
    match read {
        Ok(message) => message.is_close(),
        Err(tungstenite::Error::Io(error)) => !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        Err(_) => true,
    }
}

/// `eprintln!` panics when stderr is gone, say behind a pipe nobody reads any
/// more: on the room's thread, one line of log ended the game.
#[test]
fn a_closed_stderr_does_not_end_the_room() {
    let port = free_port();
    // Kept open: this test is about stderr.
    let (mut server, _stdout) = start(port, Some("1"));
    drop(server.0.stderr.take());
    for _ in 0..3 {
        let mut link = socket(port);
        link.close(None).unwrap();
        assert!(is_closed(link.read()));
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(server.0.try_wait().unwrap(), None, "the room died");
    socket(port);
}

#[test]
fn a_dead_room_ends_the_process() {
    let port = free_port();
    // Kept open: a server whose stdout is closed fails on its next `println!`.
    let (mut server, _stdout) = start(port, None);

    // A TV stays connected until the end.
    let mut tv = socket(port);

    let mut request = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        request,
        "POST {BOOM} HTTP/1.1\r\nHost: test\r\nContent-Length: 0\r\n\r\n"
    )
    .unwrap();

    // Finite: the process ends, without waiting for anybody.
    let start = Clock::instant();
    let status = loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the server survived its room"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(status.code(), Some(ROOM_DIED));

    // Visible: the log says what happened and why.
    let mut log = String::new();
    server
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut log)
        .unwrap();
    assert!(
        log.lines()
            .any(|line| line.starts_with("salon: the room died") && line.contains(WHY)),
        "{log}"
    );

    // The connections are closed: nothing answers any more.
    assert!(is_closed(tv.read()));
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}
