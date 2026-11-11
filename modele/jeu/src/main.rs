//! Le jeu de départ : chacun tape sur son téléphone, l'écran compte.
//!
//! `cargo run --release` compile les pages, les sert, et attend les joueurs.

mod game;

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use game::Game;
use messages::{Down, Up};
use telecommande::host::{Event, Room};
use telecommande::{Id, Role};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let pages = telecommande::pages::build(Path::new(env!("CARGO_MANIFEST_DIR")), "pages")?;
    let room = Room::<Up, Down>::open(&pages, "/t")?;
    let mut game = Game::default();
    let mut screens: BTreeSet<Id> = BTreeSet::new();
    loop {
        let Some(event) = room.next(Duration::from_secs(1)) else {
            continue;
        };
        match event {
            Event::Joined {
                id,
                role: Role::Screen,
            } => {
                let _: bool = screens.insert(id);
            }
            Event::Joined {
                id,
                role: Role::Remote,
            } => game.join(id),
            Event::Message { id, message } => game.apply(id, message),
            Event::Left { id } => {
                let _: bool = screens.remove(&id);
                game.leave(id);
            }
        }
        // Après chaque événement, chaque page reçoit ce qu'elle doit afficher.
        for &screen in &screens {
            room.send(screen, &Down::Screen(game.screen(room.remote_url())));
        }
        for player in game.players() {
            if let Some(remote) = game.remote(player) {
                room.send(player, &Down::Remote(remote));
            }
        }
    }
}
