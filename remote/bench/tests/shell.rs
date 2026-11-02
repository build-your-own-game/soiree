//! The shell on a simulator. Each test is a player's story, told to the real
//! app, and judged by what the game received.

#![cfg(target_os = "macos")]
use std::time::Duration;

use remote_bench::eventually;
use remote_bench::host::FakeHost;
use remote_bench::shell::Shell;
use remote_protocol::{DrawEvent, PhoneMessage, Remote, presets};

const SOON: Duration = Duration::from_secs(5);

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
