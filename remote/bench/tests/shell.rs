//! The shell on a simulator. Each test is a player's story, told to the real
//! app, and judged by what the game received.

#![cfg(target_os = "macos")]
use std::time::Duration;

use remote_bench::host::FakeHost;
use remote_bench::shell::Shell;
use remote_protocol::{PhoneMessage, Remote, presets};

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
