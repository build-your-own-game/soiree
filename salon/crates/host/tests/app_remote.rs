//! The iPhone app's door, end to end: a real server, a real WebSocket, and a
//! fake app that speaks only `remote-protocol`, as the real one does.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    reason = "test: its helpers are outside the #[test] functions Clippy recognizes"
)]

use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use salon_core::{DeviceEvent, Host, Ms};
use salon_host::remote::web::PlayerId;
use salon_host::remote::{
    DrawEvent, Feedback, GameMessage, Haptic, InputState, PhoneMessage, Remote, VERSION, decode,
    encode, presets,
};
use salon_host::{AppEvent, Game, Options};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const PAGE: &str = "/play";
const ADA: [u8; 16] = [7; 16];

/// Chooses a pad, and reports all it hears from the apps. Pressing A moves
/// to the buzzer with a vibration: a game changing its remote between phases.
struct Native {
    remote: Remote,
    feedback: Vec<(PlayerId, Feedback)>,
    heard: Sender<AppEvent>,
}

impl Game for Native {
    type Up = ();
    type Down = ();

    fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}

    fn remote(&self, _: PlayerId) -> Option<Remote> {
        Some(self.remote.clone())
    }

    fn on_app(&mut self, _: Ms, event: AppEvent, _: &mut Host<(), ()>) {
        if let AppEvent::Input { player, state } = &event
            && state.is_pressed(presets::pad::A)
        {
            self.remote = Remote::Pad(presets::buzzer());
            let feedback = Feedback {
                haptic: Some(Haptic::Success),
                color: None,
            };
            self.feedback.push((*player, feedback));
        }
        // The test may be over: the room outlives it, and must not panic.
        let _: Result<(), _> = self.heard.send(event);
    }

    fn feedback(&mut self) -> Vec<(PlayerId, Feedback)> {
        std::mem::take(&mut self.feedback)
    }
}

/// What every game is today: `Game`'s defaults.
struct Default;

impl Game for Default {
    type Up = ();
    type Down = ();

    fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}
}

/// Serves `game` for the rest of the test run, and gives its port.
fn serve(name: &str, game: impl Game) -> u16 {
    // Free on TCP and on UDP, which salon opens with the same number.
    let port = loop {
        let tcp = std::net::TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            break port;
        }
    };
    let dir = std::env::temp_dir().join(format!("salon-app-{name}-{port}"));
    let mut options = Options::new(port, dir.clone(), 1, "ready");
    options.data = Some(dir);
    options.app_page = Some(PAGE.to_owned());
    options.tick = Duration::from_millis(5);
    thread::spawn(move || salon_host::run(options, game));
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        thread::sleep(Duration::from_millis(5));
    }
    port
}

/// The app, as far as a game can tell.
struct App(WebSocket<MaybeTlsStream<TcpStream>>);

impl App {
    fn connect(port: u16) -> App {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let address = format!("ws://127.0.0.1:{port}/remote");
        let (socket, _) =
            tungstenite::client(address.as_str(), MaybeTlsStream::Plain(stream)).unwrap();
        App(socket)
    }

    fn say(&mut self, message: &PhoneMessage) {
        self.0.send(Message::binary(encode(message))).unwrap();
    }

    fn hello(&mut self, version: u16) {
        self.say(&PhoneMessage::Hello {
            version,
            player: ADA,
            name: "Ada".to_owned(),
        });
    }

    fn hears(&mut self) -> GameMessage {
        match self.0.read().unwrap() {
            Message::Binary(frame) => decode(&frame).unwrap(),
            other => panic!("expected a message, got {other:?}"),
        }
    }
}

fn next(heard: &Receiver<AppEvent>) -> AppEvent {
    heard.recv_timeout(Duration::from_secs(5)).unwrap()
}

#[test]
#[expect(clippy::too_many_lines, reason = "one story, told in order")]
fn a_game_chooses_its_remote_and_hears_it() {
    let (heard, hearing) = channel();
    let port = serve(
        "native",
        Native {
            remote: Remote::Pad(presets::pad()),
            feedback: Vec::new(),
            heard,
        },
    );
    let ada = PlayerId::new(ADA);

    let mut app = App::connect(port);
    app.hello(VERSION);
    assert_eq!(app.hears(), GameMessage::Show(Remote::Pad(presets::pad())));
    assert!(matches!(
        next(&hearing),
        AppEvent::Arrived { player, name: Some(name) } if player == ada && name.as_str() == "Ada"
    ));

    // Inputs reach the game; an older state does not, as the contract says.
    let state = |seq, pressed| InputState {
        seq,
        pressed,
        ..InputState::default()
    };
    app.say(&PhoneMessage::Input(state(5, 0)));
    app.say(&PhoneMessage::Input(state(4, 0)));
    app.say(&PhoneMessage::Input(state(6, 0)));
    for seq in [5, 6] {
        assert_eq!(
            next(&hearing),
            AppEvent::Input {
                player: ada,
                state: state(seq, 0)
            }
        );
    }

    // Strokes all arrive, in order.
    let strokes = [
        DrawEvent::Begin {
            stroke: 1,
            color: 0x00FF_0000,
            width: 8,
            x: 1,
            y: 2,
        },
        DrawEvent::Point {
            stroke: 1,
            x: 3,
            y: 4,
        },
        DrawEvent::End { stroke: 1 },
    ];
    for stroke in strokes {
        app.say(&PhoneMessage::Draw(stroke));
    }
    for stroke in strokes {
        assert_eq!(
            next(&hearing),
            AppEvent::Draw {
                player: ada,
                event: stroke
            }
        );
    }

    // The game changes its remote and vibrates the phone from its own state.
    app.say(&PhoneMessage::Input(state(7, 1 << presets::pad::A)));
    assert!(matches!(next(&hearing), AppEvent::Input { .. }));
    assert_eq!(
        app.hears(),
        GameMessage::Show(Remote::Pad(presets::buzzer()))
    );
    assert_eq!(
        app.hears(),
        GameMessage::Feedback(Feedback {
            haptic: Some(Haptic::Success),
            color: None
        })
    );

    // The connection drops and the app comes back: the game sees the absence,
    // and salon puts the current remote back up by itself.
    drop(app);
    assert_eq!(next(&hearing), AppEvent::Left { player: ada });
    let mut app = App::connect(port);
    app.hello(VERSION);
    assert_eq!(
        app.hears(),
        GameMessage::Show(Remote::Pad(presets::buzzer()))
    );
    assert_eq!(next(&hearing), AppEvent::Back { player: ada });

    // A new connection numbers its states from where it wants.
    app.say(&PhoneMessage::Input(state(1, 0)));
    assert_eq!(
        next(&hearing),
        AppEvent::Input {
            player: ada,
            state: state(1, 0)
        }
    );

    // A second connection of the same player takes over, without a goodbye.
    let mut second = App::connect(port);
    second.hello(VERSION);
    assert_eq!(
        second.hears(),
        GameMessage::Show(Remote::Pad(presets::buzzer()))
    );
    assert!(matches!(app.0.read(), Ok(Message::Close(_)) | Err(_)));
    second.say(&PhoneMessage::Input(state(2, 0)));
    assert_eq!(
        next(&hearing),
        AppEvent::Input {
            player: ada,
            state: state(2, 0)
        }
    );
}

#[test]
fn by_default_the_app_gets_the_phone_page_and_keeps_it() {
    let port = serve("default", Default);
    let page = || {
        GameMessage::Show(Remote::Web {
            url: PAGE.to_owned(),
        })
    };

    let mut app = App::connect(port);
    app.hello(VERSION);
    assert_eq!(app.hears(), page());
    // What it sends besides is ignored, and it can say hello again.
    app.say(&PhoneMessage::Input(InputState::default()));
    app.say(&PhoneMessage::Draw(DrawEvent::Clear));
    app.hello(VERSION);
    assert_eq!(app.hears(), page());

    // Nothing more comes.
    let MaybeTlsStream::Plain(stream) = app.0.get_ref() else {
        panic!("a plain socket");
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    assert!(app.0.read().is_err(), "the door said more than the page");
}

#[test]
fn another_version_of_the_app_is_refused_with_a_reason() {
    let port = serve("refused", Default);
    let mut app = App::connect(port);
    app.hello(VERSION + 1);
    assert!(matches!(app.hears(), GameMessage::Refuse { .. }));
}
