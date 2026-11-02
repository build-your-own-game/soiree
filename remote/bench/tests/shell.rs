//! The shell on a simulator. Each test is a player's story, told to the real
//! app, and judged by what the game received.

#![cfg(target_os = "macos")]
use std::net::TcpListener;
use std::time::Duration;

use remote_bench::eventually;
use remote_bench::host::{FakeHost, Page};
use remote_bench::salon::Salon;
use remote_bench::shell::Shell;
use remote_protocol::{DrawEvent, PhoneMessage, Remote, presets};

const SOON: Duration = Duration::from_secs(5);
/// A ping every 2 s, its answer due before the next one, and some margin.
const FROZEN: Duration = Duration::from_secs(6);
/// Long enough for the backed-off try, and a try stuck on a frozen host.
const BACK: Duration = Duration::from_secs(15);
/// A retry after 1 s, then 2 s, and the load itself.
const LOADED: Duration = Duration::from_secs(10);

#[test]
fn the_shell_says_hello_and_plays_the_pad_it_is_shown() {
    let host = FakeHost::start(Some(Remote::Pad(presets::buzzer())));
    let mut shell = Shell::launch();
    assert_eq!(shell.ask(&format!("join {}", host.page("/play"))), "ok");
    assert!(host.wait(SOON, |state| state.links == 1));
    assert!(host.wait(SOON, |state| {
        state
            .heard
            .iter()
            .any(|heard| matches!(heard.message, PhoneMessage::Input(_)))
    }));
    let (x, y) = shell.point(&format!("control {}", presets::buzzer::BUZZ));
    shell.ask(&format!("down 1 {x} {y}"));
    assert!(
        host.wait(SOON, |state| state.heard.iter().any(|heard| matches!(
            &heard.message,
            PhoneMessage::Input(input) if input.pressed != 0
        )))
    );
    shell.ask("up 1");
    assert!(matches!(
        host.messages(1).first(),
        Some(PhoneMessage::Hello { .. })
    ));
    assert_eq!(shell.ask("status"), "connected");
}

const PAGE: &str = "<!doctype html><title>page</title><body>page</body>";

/// A game that shows its page at `/play`.
fn page_game(html: &str) -> FakeHost {
    let host = FakeHost::start(Some(Remote::Web {
        url: "/play".to_owned(),
    }));
    host.with(|state| {
        state
            .routes
            .insert("/play".to_owned(), Page::Html(html.to_owned()));
    });
    host
}

/// A player on the game `host` shows at `path`, with its screen up.
fn joined(host: &FakeHost, path: &str) -> Shell {
    let mut shell = Shell::launch();
    assert_eq!(shell.ask(&format!("join {}", host.page(path))), "ok");
    assert!(
        eventually(SOON, || shell.ask("content") != "waiting"),
        "the game's remote never came"
    );
    shell
}

fn is_end(message: &PhoneMessage) -> bool {
    matches!(message, PhoneMessage::Draw(DrawEvent::End { .. }))
}

fn is_hello(message: &PhoneMessage) -> bool {
    matches!(message, PhoneMessage::Hello { .. })
}

#[test]
fn a_stroke_cut_by_the_host_reaches_the_next_socket_behind_hello() {
    let host = FakeHost::start(Some(Remote::Pad(presets::draw())));
    let mut shell = joined(&host, "/play");
    let (x, y) = shell.point("canvas");
    shell.ask(&format!("down 1 {x} {y}"));
    for step in 1..=5 {
        shell.ask(&format!("move 1 {} {y}", x + f64::from(step) * 10.0));
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(host.wait(SOON, |state| state.heard.len() > 3));
    std::thread::sleep(Duration::from_millis(200));
    // The next two tries fail too: nothing kept may be spent on them.
    host.with(|state| {
        state.refuse = 2;
        state.cut = true;
    });
    assert!(
        eventually(SOON, || shell.ask("status") == "reconnecting"),
        "the cut went unseen"
    );
    for step in 6..=10 {
        shell.ask(&format!("move 1 {} {y}", x + f64::from(step) * 10.0));
        std::thread::sleep(Duration::from_millis(30));
    }
    shell.ask("up 1");
    assert!(
        host.wait(Duration::from_secs(15), |state| state
            .heard
            .iter()
            .any(|heard| is_end(&heard.message))),
        "the stroke never ended"
    );
    let second = host.with(|state| state.links);
    let link = host.messages(second);
    assert!(link.first().is_some_and(is_hello), "{link:?}");
    assert!(link.iter().any(is_end), "{link:?}");
}

#[test]
fn a_frozen_host_is_seen_within_seconds_and_found_again() {
    let host = FakeHost::start(Some(Remote::Pad(presets::buzzer())));
    let mut shell = joined(&host, "/play");
    host.with(|state| state.frozen = true);
    assert!(
        eventually(FROZEN, || shell.ask("status") == "reconnecting"),
        "the app still believes in a frozen host"
    );
    host.with(|state| state.frozen = false);
    assert!(
        eventually(BACK, || shell.ask("status") == "connected"
            && host.with(|state| state.links) == 2),
        "the app never came back"
    );
}

/// `tungstenite` answers a ping only from `read`: salon's door must keep
/// reading, or every quiet page would drop every few seconds.
#[test]
fn a_real_salon_answers_the_pings_of_a_quiet_page() {
    let salon = Salon::start("page");
    let mut shell = Shell::launch();
    shell.ask(&format!("join {}", salon.page()));
    assert!(eventually(SOON, || shell.ask("content") != "waiting"));
    assert!(eventually(SOON, || salon.heard("arrived") == 1));
    std::thread::sleep(Duration::from_secs(9));
    assert_eq!(shell.ask("status"), "connected");
    assert_eq!(salon.heard("left"), 0, "a healthy link was dropped");
    assert_eq!(salon.heard("back"), 0, "a healthy link was dropped");
}

#[test]
fn a_frozen_salon_is_seen_and_a_stroke_it_cut_still_ends_there() {
    let salon = Salon::start("draw");
    let mut shell = Shell::launch();
    shell.ask(&format!("join {}", salon.page()));
    assert!(eventually(SOON, || shell.ask("content") == "pad"));
    let (x, y) = shell.point("canvas");
    shell.ask(&format!("down 1 {x} {y}"));
    assert!(eventually(SOON, || salon.heard("begin") == 1));
    std::thread::sleep(Duration::from_millis(200));
    salon.freeze();
    assert!(
        eventually(FROZEN, || shell.ask("status") == "reconnecting"),
        "the app still believes in a frozen salon"
    );
    shell.ask("up 1");
    std::thread::sleep(Duration::from_millis(200));
    salon.thaw();
    assert!(
        eventually(BACK, || shell.ask("status") == "connected"),
        "the app never came back"
    );
    assert!(
        eventually(SOON, || salon.heard("end") == 1),
        "salon never heard the stroke end"
    );
}

#[test]
fn a_page_that_fails_to_load_is_tried_again_with_a_word_for_the_player() {
    let host = page_game(PAGE);
    // WebKit itself tries four times in a few milliseconds.
    host.with(|state| state.broken_pages = 6);
    let mut shell = joined(&host, "/play");
    assert!(
        eventually(SOON, || shell.ask("page-failing") == "true"),
        "nothing tells the player"
    );
    assert!(
        eventually(LOADED, || shell.ask("eval document.title") == "page"),
        "the page never came: {}",
        shell.ask("eval document.title")
    );
    assert!(
        eventually(SOON, || shell.ask("page-failing") == "false"),
        "the word outlives the trouble"
    );
}

/// An address where nothing listens.
fn closed_page() -> String {
    let free = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    format!("http://127.0.0.1:{port}/play")
}

#[test]
fn a_refused_local_network_names_the_setting_that_lets_it_back() {
    let mut shell = Shell::launch();
    shell.ask(&format!("join-denied {}", closed_page()));
    assert!(
        eventually(SOON, || shell.ask("notice").contains("Réglages")),
        "the player reads: {}",
        shell.ask("notice")
    );
    assert_eq!(shell.ask("status"), "denied");
}

/// The permission cannot be refused on a simulator (S6): this is the real
/// check, saying no, and the other message.
#[test]
fn a_game_that_never_answered_is_not_called_a_lost_connection() {
    let mut shell = Shell::launch();
    shell.ask(&format!("join {}", closed_page()));
    assert!(
        eventually(SOON, || shell.ask("notice").contains("ne répond pas")),
        "the player reads: {}",
        shell.ask("notice")
    );
    assert_eq!(shell.ask("status"), "reconnecting");
}
