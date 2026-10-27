//! The demo's host, on the computer.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use game_core::protocol::{Join, Player, Up, VERSION, View};
use salon_core::{DeviceEvent, DeviceId, Host, Kind, Ms};
use salon_host::{Game, Options};

/// `GAME_PORT` changes it (the certification harness picks its own).
const PORT: u16 = 8090;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

struct Taps {
    join: Join,
    tvs: Vec<DeviceId>,
    game: game_core::Game,
    recording:
        Option<salon_host::journal::Recorder<game_core::replay::Header, game_core::replay::Entry>>,
}

impl Taps {
    fn apply(&mut self, at: Ms, input: game_core::Input) {
        let entry = game_core::replay::Entry {
            at: at.0,
            input: input.clone(),
        };
        self.game.apply(input);
        if let Some(recorder) = &mut self.recording {
            let result = recorder
                .record(entry)
                .map_err(|error| error.to_string())
                .and_then(|()| game_core::replay::fingerprint(&self.game))
                .and_then(|hash| recorder.checkpoint(hash).map_err(|error| error.to_string()));
            if let Err(error) = result {
                eprintln!("journal stopped: {error}");
                self.recording = None;
            }
        }
    }
    fn show_everyone(&self, host: &mut Host<Up, View>) {
        let mut players: Vec<Player> = self
            .game
            .players()
            .values()
            .map(|seat| Player {
                name: seat.name.clone(),
                taps: seat.taps,
                here: seat.here,
                rtt: None,
            })
            .collect();
        for (device, player) in self.game.players().keys().zip(players.iter_mut()) {
            player.rtt = host.device(DeviceId(*device)).and_then(|info| info.rtt);
        }
        let taps = players
            .iter()
            .fold(0_u32, |sum, player| sum.saturating_add(player.taps));
        for tv in &self.tvs {
            let view = View {
                taps,
                players: players.clone(),
                join: Some(self.join.clone()),
                mine: None,
            };
            // A state replaces the last one: a device that missed it gets the next.
            let _: Result<(), salon_core::SendError> = host.show(*tv, &view);
        }
        for (device, player) in self.game.players() {
            let view = View {
                taps,
                players: players.clone(),
                join: None,
                mine: Some(player.taps),
            };
            let _: Result<(), salon_core::SendError> = host.show(DeviceId(*device), &view);
        }
    }
}

impl Game for Taps {
    type Up = Up;
    type Down = View;

    fn on(&mut self, now: Ms, event: DeviceEvent<Up>, host: &mut Host<Up, View>) {
        match event {
            DeviceEvent::Arrived {
                device,
                kind: Kind::Tv,
                ..
            } => self.tvs.push(device),
            DeviceEvent::Arrived { device, name, .. } => {
                let name =
                    name.map_or_else(|| format!("Joueur {}", device.0), |name| name.to_string());
                // The last seat taken: the next phone is turned away clearly.
                if self.game.players().len().saturating_add(1) >= game_core::MAX_PLAYERS {
                    host.close();
                }
                self.apply(
                    now,
                    game_core::Input::Join {
                        player: device.0,
                        name,
                    },
                );
                // Several handshakes may be delivered together before the
                // room closes. The simulation's capacity remains authoritative.
                if !self.game.players().contains_key(&device.0) {
                    host.forget(device);
                }
            }
            DeviceEvent::Left { device, .. } | DeviceEvent::Back { device } => {
                let here = host
                    .device(device)
                    .is_some_and(|info| info.presence != salon_core::Presence::Absent);
                self.apply(
                    now,
                    game_core::Input::Presence {
                        player: device.0,
                        here,
                    },
                );
            }
            DeviceEvent::Event {
                device,
                event: Up::Tap,
                ..
            } => {
                self.apply(now, game_core::Input::Tap { player: device.0 });
            }
            DeviceEvent::State { .. } => {}
        }
        self.show_everyone(host);
    }

    fn diagnostic(&mut self, now: Ms, report: &salon_core::journal::Diagnostic) {
        if let Some(recorder) = &mut self.recording
            && let Err(error) =
                recorder.marker(now, format!("{}: {}", report.page(), report.message()))
        {
            eprintln!("journal marker failed: {error}");
            self.recording = None;
        }
    }

    fn tick(&mut self, _now: Ms, host: &mut Host<Up, View>) {
        // Refreshes the round trips on screen.
        self.show_everyone(host);
    }
}

fn build(root: &Path, dist: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let web_package = env!("CARGO_PKG_NAME").replace("-host", "-web");
    let status = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--release",
            "-p",
            &web_package,
            "--target",
            "wasm32-unknown-unknown",
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("the page did not build".to_owned());
    }
    let status = Command::new("wasm-bindgen")
        .args([
            "--target",
            "web",
            "--no-typescript",
            "--out-name",
            "game",
            "--out-dir",
        ])
        .arg(dist.join("pkg"))
        .arg(
            root.join("target/wasm32-unknown-unknown/release")
                .join(format!("{}.wasm", web_package.replace('-', "_"))),
        )
        .status()
        .map_err(|error| format!("{error} (cargo install wasm-bindgen-cli --version 0.2.129)"))?;
    if !status.success() {
        return Err("wasm-bindgen failed".to_owned());
    }
    std::fs::create_dir_all(dist).map_err(|error| error.to_string())?;
    for file in ["index.html", "style.css"] {
        std::fs::copy(root.join("web").join(file), dist.join(file))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().collect();
    if let Some(at) = args.iter().position(|arg| arg == "--replay") {
        let Some(path) = args.get(at.saturating_add(1)) else {
            eprintln!("--replay needs a journal path");
            return ExitCode::FAILURE;
        };
        let result = salon_host::journal::Journal::<
            game_core::replay::Header,
            game_core::replay::Entry,
        >::read(path)
        .map_err(|error| error.to_string())
        .and_then(|journal| {
            println!(
                "Replay sources: recorded {}, current {}",
                journal.header.version, VERSION
            );
            game_core::replay::replay(&journal.header, &journal.entries, &journal.fingerprint)
        });
        return match result {
            Ok(_) => {
                println!("Replay verified");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dist = root.join("dist");
    if !std::env::args().any(|arg| arg == "--no-build")
        && let Err(error) = build(&root, &dist)
    {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    let port = std::env::var("GAME_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(PORT);
    let url = salon_host::join_url(port, "/play");
    println!("  phones:             {url}");
    let mut options = Options::new(port, dist, VERSION, "Game ready");
    options
        .pages
        .push(("/play".to_owned(), "index.html".to_owned()));
    let data = std::env::var_os("GAME_DATA").map_or_else(|| root.join("data"), PathBuf::from);
    options.data = Some(data.clone());
    options.app_page = Some("/play".to_owned());
    options.tick = std::time::Duration::from_secs(1);
    let mut recording = salon_host::journal::Recorder::start(
        data.join("journals"),
        "game",
        game_core::replay::Header::default(),
    )
    .map_err(|error| eprintln!("journal unavailable: {error}"))
    .ok();
    if let Some(recorder) = &mut recording {
        let result = game_core::replay::fingerprint(&game_core::Game::default())
            .and_then(|hash| recorder.checkpoint(hash).map_err(|error| error.to_string()));
        if let Err(error) = result {
            eprintln!("journal unavailable: {error}");
            recording = None;
        }
    }
    let game = Taps {
        join: Join {
            qr: salon_host::qr(&url).unwrap_or_default(),
            url,
        },
        tvs: Vec::new(),
        game: game_core::Game::default(),
        recording,
    };
    match salon_host::run(options, game) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
