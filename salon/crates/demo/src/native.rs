//! The demo's host, on the computer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::protocol::{Player, Up, VERSION, View};
use salon_core::{DeviceEvent, DeviceId, Host, Kind, Ms};
use salon_host::{Game, Options};

/// `SALON_DEMO_PORT` changes it (the certification harness picks its own).
const PORT: u16 = 8095;
/// Phones the demo takes (`salon/cert.json` says the same).
const MAX_PHONES: usize = 15;

struct Taps {
    join: (String, String),
    tvs: Vec<DeviceId>,
    players: BTreeMap<DeviceId, Player>,
}

impl Taps {
    fn show_everyone(&self, host: &mut Host<Up, View>) {
        let mut players: Vec<Player> = self.players.values().cloned().collect();
        for (device, player) in self.players.keys().zip(players.iter_mut()) {
            player.rtt = host.device(*device).and_then(|info| info.rtt);
        }
        let taps = players.iter().map(|player| player.taps).sum();
        for tv in &self.tvs {
            let view = View {
                taps,
                players: players.clone(),
                join: Some(self.join.clone()),
                mine: None,
            };
            let _ = host.show(*tv, &view);
        }
        for (device, player) in &self.players {
            let view = View {
                taps,
                players: players.clone(),
                join: None,
                mine: Some(player.taps),
            };
            let _ = host.show(*device, &view);
        }
    }
}

impl Game for Taps {
    type Up = Up;
    type Down = View;

    fn on(&mut self, _now: Ms, event: DeviceEvent<Up>, host: &mut Host<Up, View>) {
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
                if self.players.len() + 1 >= MAX_PHONES {
                    host.set_open(false);
                }
                self.players.insert(
                    device,
                    Player {
                        name,
                        taps: 0,
                        here: true,
                        rtt: None,
                    },
                );
            }
            DeviceEvent::Left { device, .. } | DeviceEvent::Back { device } => {
                let here = host
                    .device(device)
                    .is_some_and(|info| info.presence != salon_core::Presence::Absent);
                if let Some(player) = self.players.get_mut(&device) {
                    player.here = here;
                }
            }
            DeviceEvent::Event { device, event, .. } => {
                if let Some(player) = self.players.get_mut(&device) {
                    match event {
                        Up::Tap => player.taps += 1,
                        Up::Name(name) => {
                            if let Some(name) = salon_core::Name::new(&name) {
                                player.name = name.to_string();
                            }
                        }
                    }
                }
            }
            DeviceEvent::State { .. } => {}
        }
        self.show_everyone(host);
    }

    fn tick(&mut self, _now: Ms, host: &mut Host<Up, View>) {
        // Refreshes the round trips on screen.
        self.show_everyone(host);
    }
}

fn build(root: &Path, dist: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--release",
            "-p",
            "salon-demo-page",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("the page did not build".to_owned());
    }
    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(dist.join("pkg"))
        .arg(root.join("target/wasm32-unknown-unknown/release/salon_demo_page.wasm"))
        .status()
        .map_err(|error| format!("{error} (cargo install wasm-bindgen-cli --version 0.2.129)"))?;
    if !status.success() {
        return Err("wasm-bindgen failed".to_owned());
    }
    std::fs::create_dir_all(dist).map_err(|error| error.to_string())?;
    std::fs::copy(root.join("demo-web/index.html"), dist.join("index.html"))
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dist = root.join("target/demo-dist");
    if !std::env::args().any(|arg| arg == "--no-build")
        && let Err(error) = build(&root, &dist)
    {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    let port = std::env::var("SALON_DEMO_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(PORT);
    let url = salon_host::join_url(port, "/play");
    let qr = salon_host::qr_svg(&url, "#12131a", "#ffffff").unwrap_or_default();
    println!("  phones:             {url}");
    let mut options = Options::new(port, dist, VERSION, "Salon demo ready");
    options
        .pages
        .push(("/play".to_owned(), "index.html".to_owned()));
    options.data = Some(
        std::env::var_os("SALON_DEMO_DATA")
            .map_or_else(|| root.join("target/demo-data"), PathBuf::from),
    );
    options.app_page = Some("/play".to_owned());
    options.tick = std::time::Duration::from_secs(1);
    let game = Taps {
        join: (url, qr),
        tvs: Vec::new(),
        players: BTreeMap::new(),
    };
    match salon_host::run(options, game) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
