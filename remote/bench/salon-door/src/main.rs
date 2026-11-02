//! A real salon with a real `/remote` door, in its own process so that a test
//! can freeze it: `salon-door <port> <dir> pad|draw|page`. It prints what the
//! game hears from the app, one line each, on stdout.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use remote_protocol::web::PlayerId;
use remote_protocol::{DrawEvent, Remote, presets};
use salon_core::{DeviceEvent, Host, Ms};
use salon_host::{AppEvent, Game, Options};

struct Door {
    remote: Option<Remote>,
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _: std::io::Result<()> = writeln!(out, "{line}").and_then(|()| out.flush());
}

impl Game for Door {
    type Up = ();
    type Down = ();

    fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}

    fn remote(&self, _: PlayerId) -> Option<Remote> {
        self.remote.clone()
    }

    fn on_app(&mut self, _: Ms, event: AppEvent, _: &mut Host<(), ()>) {
        match event {
            AppEvent::Arrived { .. } => say("arrived"),
            AppEvent::Left { .. } => say("left"),
            AppEvent::Back { .. } => say("back"),
            AppEvent::Input { .. } => {}
            AppEvent::Draw { event, .. } => say(match event {
                DrawEvent::Begin { .. } => "begin",
                DrawEvent::Point { .. } => "point",
                DrawEvent::End { .. } => "end",
                DrawEvent::Undo => "undo",
                DrawEvent::Clear => "clear",
            }),
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(port), Some(dir), Some(kind)) = (
        args.next().and_then(|port| port.parse().ok()),
        args.next(),
        args.next(),
    ) else {
        eprintln!("usage: salon-door <port> <dir> pad|draw|page");
        return ExitCode::FAILURE;
    };
    let remote = match kind.as_str() {
        "pad" => Some(Remote::Pad(presets::pad())),
        "draw" => Some(Remote::Pad(presets::draw())),
        _ => None,
    };
    let dir = std::path::PathBuf::from(dir);
    let mut options = Options::new(port, dir.clone(), 1, "ready");
    options.data = Some(dir);
    options.app_page = Some("/play".to_owned());
    options.tick = Duration::from_millis(50);
    match salon_host::run(options, Door { remote }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
