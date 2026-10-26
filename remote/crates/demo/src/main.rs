//! `cargo run -p remote-demo`: a fake game for the remote.
//!
//! It serves the remote's WebSocket at `/remote`, prints a QR code to scan
//! with the app, and shows every state and drawing step phones send. Keys
//! switch the remote every phone shows, the way a game does between phases.
//! It also serves `/free`, a page for the free path, to try the app's bridge.

use std::fmt::Write as _;
use std::io::{self, BufRead, Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use remote_protocol::{
    DrawEvent, Feedback, GameMessage, Haptic, InputState, PATH, PhoneMessage, Remote, VERSION,
    decode, encode, presets,
};
use tungstenite::{Message, WebSocket};

const PORT: u16 = 8095;

/// What every phone shows, and how to reach each of them.
struct Shared {
    remote: Remote,
    phones: Vec<Sender<GameMessage>>,
}

type State = Arc<Mutex<Shared>>;

fn lock(state: &State) -> MutexGuard<'_, Shared> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("remote-demo: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, PORT))
        .map_err(|e| format!("port {PORT}: {e}"))?;
    let page = format!("http://{}:{PORT}/play", lan_ip());
    let local = format!("http://127.0.0.1:{PORT}/play");
    println!("{}", qr(&page));
    println!("Faux jeu sur {page}");
    println!("Scanne ce QR avec l'app Télécommande. Dans le simulateur :");
    println!("  xcrun simctl openurl booted '{}'\n", join_link(&local));
    println!("Touches, puis Entrée :");
    for (key, (name, _)) in (1..).zip(presets::all()) {
        println!("  {key}  preset {name}");
    }
    println!(
        "  w  page libre (/free)\n  s  vibration « succès »\n  x  vibration « attention »\n  q  quitter\n"
    );

    let state: State = Arc::new(Mutex::new(Shared {
        remote: Remote::Pad(presets::pad()),
        phones: Vec::new(),
    }));
    let accepting = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = Arc::clone(&accepting);
            thread::spawn(move || {
                if let Err(e) = serve(stream, &state) {
                    println!("\n× {e}");
                }
            });
        }
    });

    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let message = match line.trim() {
            "q" => return Ok(()),
            "w" => GameMessage::Show(Remote::Web {
                url: "/free".into(),
            }),
            "s" => GameMessage::Feedback(Feedback {
                haptic: Some(Haptic::Success),
                color: Some(0x002B_B673),
            }),
            "x" => GameMessage::Feedback(Feedback {
                haptic: Some(Haptic::Warning),
                color: Some(0x00E6_3946),
            }),
            key => {
                let chosen = key
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|i| presets::all().into_iter().nth(i));
                let Some((name, layout)) = chosen else {
                    println!("? {key}");
                    continue;
                };
                println!("→ preset {name}");
                GameMessage::Show(Remote::Pad(layout))
            }
        };
        let mut shared = lock(&state);
        if let GameMessage::Show(remote) = &message {
            shared.remote = remote.clone();
        }
        shared
            .phones
            .retain(|phone| phone.send(message.clone()).is_ok());
    }
    Ok(())
}

/// One TCP connection: the remote's WebSocket, or a page.
fn serve(stream: TcpStream, state: &State) -> Result<(), String> {
    let mut head = [0; 1024];
    let read = stream.peek(&mut head).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(head.get(..read).unwrap_or_default()).into_owned();
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    if path.starts_with(PATH) {
        return phone(stream, state);
    }
    let (status, kind, body) = if path == "/free" {
        ("200 OK", "text/html; charset=utf-8", FREE_PAGE.to_string())
    } else if let Some(query) = path.strip_prefix("/free-input?") {
        println!("\n← page libre : {query}");
        ("204 No Content", "text/plain", String::new())
    } else if path == "/" || path.starts_with("/play") {
        let link = join_link(&format!("http://{}:{PORT}/play", lan_ip()));
        (
            "200 OK",
            "text/html; charset=utf-8",
            PLAY_PAGE.replace("{link}", &link),
        )
    } else {
        ("404 Not Found", "text/plain", "introuvable".to_string())
    };
    respond(stream, status, kind, &body)
}

fn respond(mut stream: TcpStream, status: &str, kind: &str, body: &str) -> Result<(), String> {
    // Read the request away so the browser sees a clean close.
    let mut request = [0; 4096];
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| e.to_string())?;
    let _ = stream.read(&mut request);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|e| e.to_string())
}

/// A phone on `/remote`: hello, the current remote, then its states.
fn phone(stream: TcpStream, state: &State) -> Result<(), String> {
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    let mut socket = tungstenite::accept(stream).map_err(|e| e.to_string())?;
    socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|e| e.to_string())?;
    let (sender, outbox) = mpsc::channel();
    lock(state).phones.push(sender);
    let mut name = String::from("?");
    let mut last: Option<InputState> = None;
    loop {
        flush(&mut socket, &outbox)?;
        let frame = match socket.read() {
            Ok(Message::Binary(frame)) => frame,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => return Err(format!("{name} : {e}")),
        };
        match decode::<PhoneMessage>(&frame) {
            Ok(PhoneMessage::Hello {
                version, name: who, ..
            }) => {
                name = if who.is_empty() {
                    "un téléphone".into()
                } else {
                    who
                };
                if version == VERSION {
                    println!("\n+ {name} a rejoint");
                    let remote = lock(state).remote.clone();
                    send(&mut socket, &GameMessage::Show(remote))?;
                } else {
                    let reason = format!("version {version}, ce jeu parle la version {VERSION}");
                    send(&mut socket, &GameMessage::Refuse { reason })?;
                }
            }
            Ok(PhoneMessage::Input(input)) => {
                let changed = last.as_ref().is_none_or(|l| {
                    InputState {
                        seq: input.seq,
                        ..l.clone()
                    } != input
                });
                if changed {
                    print!("\r\x1b[2K{name} : {}", describe(&input));
                    let _ = io::stdout().flush();
                }
                last = Some(input);
            }
            Ok(PhoneMessage::Draw(event)) => {
                if !matches!(event, DrawEvent::Point { .. }) {
                    println!("\n✎ {name} : {event:?}");
                }
            }
            Err(e) => println!("\n× {name} : trame illisible ({e})"),
        }
    }
    println!("\n- {name} est parti");
    Ok(())
}

fn flush(socket: &mut WebSocket<TcpStream>, outbox: &Receiver<GameMessage>) -> Result<(), String> {
    while let Ok(message) = outbox.try_recv() {
        send(socket, &message)?;
    }
    Ok(())
}

fn send(socket: &mut WebSocket<TcpStream>, message: &GameMessage) -> Result<(), String> {
    socket
        .send(Message::binary(encode(message)))
        .map_err(|e| e.to_string())
}

/// One line: pressed ids, sticks, tilt, pointer.
fn describe(input: &InputState) -> String {
    let mut line = format!("#{:<5}", input.seq);
    let pressed: Vec<String> = (0..32)
        .filter(|id| input.is_pressed(*id))
        .map(|id| id.to_string())
        .collect();
    let _ = write!(line, " appuyés [{}]", pressed.join(","));
    for stick in &input.sticks {
        let (x, y) = input.stick(stick.id);
        let _ = write!(line, "  stick {} {x:+.2} {y:+.2}", stick.id);
    }
    if input.tilt != 0 {
        let _ = write!(line, "  volant {:+.2}", input.tilt());
    }
    if let Some((x, y)) = input.pointer() {
        let _ = write!(line, "  curseur {x:.2} {y:.2}");
    }
    line
}

/// The address phones on the Wi-Fi reach: the interface of the default
/// route, found by "connecting" a UDP socket (nothing is sent).
fn lan_ip() -> IpAddr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket
                .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
                .map(|()| socket)
        })
        .and_then(|socket| socket.local_addr())
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |address| address.ip())
}

/// The link that opens the app on a game, from the game's page address.
fn join_link(page: &str) -> String {
    let encoded: String = page
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("telecommande://join?url={encoded}")
}

fn qr(text: &str) -> String {
    QrCode::new(text.as_bytes()).map_or_else(
        |_| text.to_string(),
        |code| code.render::<Dense1x2>().quiet_zone(true).build(),
    )
}

/// What a phone without the app sees: where to get it, and the link that
/// opens it on this game.
const PLAY_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Faux jeu</title><body style="font:18px -apple-system,sans-serif;background:#14161b;color:#fff;display:grid;place-items:center;height:100vh;margin:0;text-align:center">
<div><p>Faux jeu de la Télécommande</p><p><a style="color:#3a86ff;font-size:22px" href="{link}">Ouvrir dans l'app Télécommande</a></p></div>"#;

/// The free path: the game's own page, here a single button. It calls the
/// app's bridge when it is there and tells the game through plain HTTP.
const FREE_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>Page libre</title>
<style>html,body{margin:0;height:100%;background:#14161b;color:#fff;font:600 18px -apple-system,sans-serif;touch-action:none;-webkit-user-select:none;user-select:none}
body{display:grid;place-items:center}#b{width:46vmin;height:46vmin;border-radius:50%;background:radial-gradient(circle at 35% 30%,#ff8a5c,#e2471f);display:grid;place-items:center;font-size:6vmin;box-shadow:0 1vmin 0 #8c2a10}
#b.on{transform:scale(.94);filter:brightness(.85)}#m{position:fixed;bottom:14px;width:100%;text-align:center;opacity:.6;font:13px ui-monospace,monospace}</style>
<div id="b">PAGE LIBRE</div><div id="m"></div>
<script>
const app = window.telecommande, b = document.getElementById("b"), m = document.getElementById("m");
let taps = 0;
b.addEventListener("pointerdown", () => { b.classList.add("on"); app && app.vibrate("press"); fetch("/free-input?tap=" + (++taps)); });
for (const e of ["pointerup", "pointercancel", "pointerleave"]) b.addEventListener(e, () => b.classList.remove("on"));
if (app) app.motion(s => { m.textContent = "gravité " + s.gravity.map(v => v.toFixed(2)).join(" "); });
else m.textContent = "Hors de l'app : ni vibration ni capteurs";
</script>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_join_link_carries_the_page_encoded() {
        assert_eq!(
            join_link("http://192.168.1.5:8095/play?room=AB"),
            "telecommande://join?url=http%3A%2F%2F192.168.1.5%3A8095%2Fplay%3Froom%3DAB"
        );
    }

    #[test]
    fn a_state_reads_as_one_line() {
        let input = InputState {
            seq: 7,
            pressed: 0b11,
            sticks: vec![remote_protocol::StickValue {
                id: 3,
                x: 127,
                y: 0,
            }],
            tilt: 0,
            pointer: None,
        };
        assert_eq!(
            describe(&input),
            "#7     appuyés [0,1]  stick 3 +1.00 +0.00"
        );
    }
}
