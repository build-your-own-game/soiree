//! The shell on a simulator. Each test is a player's story, told to the real
//! app, and judged by what the game received.

#![cfg(target_os = "macos")]
use std::net::{TcpListener, UdpSocket};
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
    settle(&mut shell);
    shell
}

/// What the screen shows has its final size: nothing lifts a finger now.
fn settle(shell: &mut Shell) {
    if shell.ask("content") != "pad" {
        return;
    }
    // The safe area can follow the rotation: the same answer for a while.
    let (mut last, mut same) = (String::new(), 0);
    let settled = eventually(SOON, || {
        let turned = shell.ask("turned");
        same = if turned == last && turned != "no" {
            same + 1
        } else {
            0
        };
        last = turned;
        same >= 12
    });
    assert!(settled, "the pad never settled: {last}");
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
    assert!(
        link.iter()
            .any(|message| matches!(message, PhoneMessage::Draw(DrawEvent::Point { .. }))),
        "the points drawn during the cut are gone: {link:?}"
    );
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
    settle(&mut shell);
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

fn post(shell: &mut Shell, message: &str) {
    let script = format!("window.webkit.messageHandlers.telecommande.postMessage({message}); 1");
    assert_eq!(shell.ask(&format!("eval {script}")), "1");
}

#[test]
fn a_page_whose_process_keeps_dying_comes_back_with_its_sockets_free() {
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    udp.set_read_timeout(Some(SOON)).unwrap();
    let port = udp.local_addr().unwrap().port();
    let host = page_game(PAGE);
    let mut shell = joined(&host, "/play");
    assert!(eventually(LOADED, || shell.ask("eval document.title") == "page"));
    for id in 1..=8 {
        post(
            &mut shell,
            &format!("{{datagram: {{op: 'open', id: {id}, port: {port}}}}}"),
        );
    }
    for death in 2..=4 {
        shell.ask("kill-page");
        assert!(
            host.wait(LOADED, |state| state.requests.len() >= death),
            "death {}: the page stayed dead, {:?}",
            death - 1,
            host.with(|state| state.requests.clone())
        );
        assert!(eventually(LOADED, || shell.ask("eval document.title") == "page"));
        assert!(eventually(SOON, || shell.ask("page-failing") == "false"));
    }
    post(
        &mut shell,
        &format!("{{datagram: {{op: 'open', id: 9, port: {port}}}}}"),
    );
    post(
        &mut shell,
        "{datagram: {op: 'send', id: 9, data: [1, 2, 3]}}",
    );
    let mut received = [0; 8];
    let got = udp.recv(&mut received);
    assert!(
        matches!(got, Ok(3)),
        "the dead page's eight sockets are still taken: {got:?}"
    );
}

#[test]
fn a_mute_address_costs_a_try_seconds_and_not_a_minute() {
    let host = FakeHost::start(None);
    host.with(|state| state.silent = true);
    let mut shell = Shell::launch();
    shell.ask(&format!("join {}", host.page("/play")));
    assert!(host.wait(SOON, |state| state.accepted == 1));
    assert!(
        host.wait(Duration::from_secs(8), |state| state.accepted == 2),
        "the first try is still waiting"
    );
}

#[test]
fn a_tap_shorter_than_a_refresh_reaches_the_game() {
    let host = FakeHost::start(Some(Remote::Pad(presets::buzzer())));
    let mut shell = joined(&host, "/play");
    let (x, y) = shell.point(&format!("control {}", presets::buzzer::BUZZ));
    shell.ask(&format!("tap 1 {x} {y}"));
    assert!(
        host.wait(SOON, |state| state.heard.iter().any(|heard| matches!(
            &heard.message,
            PhoneMessage::Input(input) if input.pressed != 0
        ))),
        "the game saw nothing"
    );
}

#[test]
fn a_reconnection_keeps_the_drawing_on_the_phone() {
    let host = FakeHost::start(Some(Remote::Pad(presets::draw())));
    let mut shell = joined(&host, "/play");
    let (x, y) = shell.point("canvas");
    shell.ask(&format!("down 1 {x} {y}"));
    shell.ask("up 1");
    assert!(eventually(SOON, || shell.ask("strokes") == "1"));
    assert!(host.wait(SOON, |state| state.heard.iter().any(|h| is_end(&h.message))));
    host.with(|state| state.cut = true);
    assert!(host.wait(BACK, |state| state.links == 2));
    assert!(eventually(BACK, || shell.ask("status") == "connected"));
    assert_eq!(shell.ask("strokes"), "1", "the drawing is gone");
    shell.ask(&format!("down 2 {x} {y}"));
    shell.ask("up 2");
    assert!(
        host.wait(SOON, |state| state.heard.iter().any(|heard| matches!(
            heard.message,
            PhoneMessage::Draw(DrawEvent::Begin { stroke: 1, .. })
        ))),
        "the next stroke took a number already used"
    );
}

#[test]
fn a_thumb_left_on_the_stick_still_steers_after_a_cut() {
    let host = FakeHost::start(Some(Remote::Pad(presets::pad())));
    let mut shell = joined(&host, "/play");
    let (x, y) = shell.point("stick");
    shell.ask(&format!("down 1 {x} {y}"));
    shell.ask(&format!("move 1 {} {y}", x + 10.0));
    assert!(host.wait(SOON, |state| state.heard.len() > 2));
    host.with(|state| state.cut = true);
    assert!(host.wait(BACK, |state| state.links == 2));
    assert!(eventually(BACK, || shell.ask("status") == "connected"));
    shell.ask(&format!("move 1 {} {y}", x + 40.0));
    assert!(
        host.wait(SOON, |state| state
            .heard
            .iter()
            .any(|heard| heard.link == 2
                && matches!(&heard.message, PhoneMessage::Input(input)
                if input.sticks.iter().any(|stick| stick.x > 50)))),
        "the thumb is dead: {:?}",
        host.messages(1).last()
    );
}

const LISTENING: &str = "<!doctype html><title>page</title><script>\
    window.heard = [];\
    addEventListener('telecommande:orientation', e => heard.push(e.detail));\
    </script>";

#[test]
fn a_half_turn_reaches_the_page() {
    let host = page_game(LISTENING);
    let mut shell = joined(&host, "/play");
    assert!(eventually(LOADED, || shell.ask("eval document.title") == "page"));
    assert!(eventually(SOON, || shell
        .ask("orientation")
        .starts_with("landscape")));
    // The first turn, to landscape, can still be on its way.
    std::thread::sleep(Duration::from_secs(1));
    let before = shell.ask("orientation");
    shell.ask("eval window.heard = []; 1");
    shell.ask("half-turn");
    assert!(
        eventually(SOON, || shell.ask("orientation") != before),
        "the scene did not turn"
    );
    let after = shell.ask("orientation");
    assert!(
        eventually(SOON, || shell
            .ask("eval window.heard.join()")
            .contains(&after)),
        "the page was not told: {}",
        shell.ask("eval window.heard.join()")
    );
    assert_eq!(shell.ask("eval window.telecommande.orientation"), after);
}

#[test]
fn a_half_turn_reaches_the_wheel() {
    let host = FakeHost::start(Some(Remote::Pad(presets::wheel())));
    let mut shell = joined(&host, "/play");
    let before = shell.ask("landscape-left");
    assert_eq!(
        before,
        (shell.ask("orientation") == "landscapeLeft").to_string()
    );
    let turned = shell.ask("orientation");
    shell.ask("half-turn");
    assert!(
        eventually(SOON, || shell.ask("orientation") != turned),
        "the scene did not turn"
    );
    assert!(
        eventually(SOON, || shell.ask("landscape-left") != before),
        "the wheel still reads the old side"
    );
}

#[test]
fn only_the_page_itself_reaches_the_bridge() {
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    udp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = udp.local_addr().unwrap().port();
    let frame = format!(
        "<!doctype html><title>frame</title><script>\
         const app = window.webkit.messageHandlers.telecommande;\
         app.postMessage({{datagram: {{op: 'open', id: 1, port: {port}}}}});\
         app.postMessage({{datagram: {{op: 'send', id: 1, data: [66]}}}});\
         document.title = 'sent';</script>"
    );
    let host = page_game("");
    let elsewhere = format!("http://localhost:{}", host.port);
    let page =
        format!("<!doctype html><title>page</title><iframe src=\"{elsewhere}/frame\"></iframe>");
    host.with(|state| {
        state.routes.insert("/play".to_owned(), Page::Html(page));
        state.routes.insert("/frame".to_owned(), Page::Html(frame));
    });
    let mut shell = joined(&host, "/play");
    assert!(eventually(LOADED, || shell.ask("eval document.title") == "page"));
    assert!(host.wait(SOON, |state| state.requests.iter().any(|r| r == "/frame")));

    // The page's own message goes through: the refusal is about the frame.
    post(
        &mut shell,
        &format!("{{datagram: {{op: 'open', id: 2, port: {port}}}}}"),
    );
    post(&mut shell, "{datagram: {op: 'send', id: 2, data: [65]}}");
    let mut received = [0; 8];
    assert_eq!(udp.recv(&mut received).unwrap(), 1);
    assert_eq!(received[0], 65, "another origin's frame reached the bridge");
}

#[test]
fn a_page_stays_on_its_origin_and_takes_the_identity_nowhere_else() {
    let host = page_game(PAGE);
    let away = format!("http://localhost:{}/away", host.port);
    host.with(|state| {
        let routes = &mut state.routes;
        routes.insert("/hop".to_owned(), Page::Redirect(away.clone()));
        routes.insert("/away".to_owned(), Page::Html(PAGE.to_owned()));
        routes.insert("/next".to_owned(), Page::Html(PAGE.to_owned()));
    });
    let mut shell = joined(&host, "/play");
    assert!(eventually(LOADED, || shell.ask("eval document.title") == "page"));

    shell.ask(&format!("eval location.href = '{away}'; 1"));
    shell.ask("eval location.href = '/hop'; 1");
    assert!(host.wait(SOON, |state| state.requests.iter().any(|r| r == "/hop")));
    std::thread::sleep(Duration::from_secs(1));
    assert!(
        !host.with(|state| state.requests.iter().any(|r| r == "/away")),
        "the page left its origin, player id in hand"
    );

    shell.ask("eval location.href = '/next'; 1");
    assert!(
        host.wait(SOON, |state| state.requests.iter().any(|r| r == "/next")),
        "a page must still move around its own origin"
    );
    assert!(eventually(LOADED, || shell
        .ask("eval location.pathname")
        .contains("next")));
    assert_eq!(shell.ask("eval typeof window.telecommande"), "object");
}
