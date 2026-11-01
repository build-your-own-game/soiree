//! A dead room must not leave a server that still answers: phones and the TV
//! would retry against it for ever. A real game panics in a real server, in a
//! child process, so that its exit code and its log can be read.

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

#[test]
fn a_dead_room_ends_the_process() {
    // Free on TCP and on UDP, which salon opens with the same number.
    let port = loop {
        let tcp = std::net::TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            break port;
        }
    };
    let mut server = Server(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "serve_a_doomed_game", "--ignored", "--nocapture"])
            .env(PORT, port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    // Kept open: a server whose stdout is closed fails on its next `println!`.
    let mut stdout = BufReader::new(server.0.stdout.take().unwrap()).lines();
    let ready = stdout
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.contains("doomed ready"));
    assert!(ready, "the server did not start");

    // A TV stays connected until the end.
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let address = format!("ws://127.0.0.1:{port}{}", route::SOCKET);
    let (mut tv, _) = tungstenite::client(address.as_str(), stream).unwrap();

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
    assert!(tv.read().is_err() || matches!(tv.read(), Ok(tungstenite::Message::Close(_))));
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}
