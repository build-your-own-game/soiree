//! Les règles : chacun tape, l'écran compte. Rien ici ne connaît le réseau.

use std::collections::BTreeMap;

use messages::{Player, Remote, Screen, Up};
use telecommande::Id;

const NAME_MAX: usize = 16;

#[derive(Default)]
pub struct Game {
    seats: BTreeMap<Id, Seat>,
}

struct Seat {
    name: String,
    taps: u32,
    here: bool,
}

impl Game {
    /// Un téléphone arrive ou revient : il garde sa place et son score.
    pub fn join(&mut self, id: Id) {
        let number = self.seats.len().saturating_add(1);
        self.seats
            .entry(id)
            .or_insert_with(|| Seat {
                name: format!("Joueur {number}"),
                taps: 0,
                here: true,
            })
            .here = true;
    }

    pub fn leave(&mut self, id: Id) {
        if let Some(seat) = self.seats.get_mut(&id) {
            seat.here = false;
        }
    }

    pub fn apply(&mut self, id: Id, up: Up) {
        let Some(seat) = self.seats.get_mut(&id) else {
            return;
        };
        match up {
            Up::Name(name) => {
                let name: String = name.trim().chars().take(NAME_MAX).collect();
                if !name.is_empty() {
                    seat.name = name;
                }
            }
            Up::Tap => seat.taps = seat.taps.saturating_add(1),
        }
    }

    pub fn players(&self) -> impl Iterator<Item = Id> + '_ {
        self.seats.keys().copied()
    }

    pub fn screen(&self, join_url: &str) -> Screen {
        Screen {
            join_url: join_url.to_owned(),
            players: self
                .seats
                .values()
                .map(|seat| Player {
                    name: seat.name.clone(),
                    taps: seat.taps,
                    here: seat.here,
                })
                .collect(),
        }
    }

    pub fn remote(&self, id: Id) -> Option<Remote> {
        self.seats.get(&id).map(|seat| Remote {
            name: seat.name.clone(),
            taps: seat.taps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_keeps_name_and_score_across_a_disconnection() {
        let mut game = Game::default();
        let ada = Id(1);
        game.join(ada);
        game.apply(ada, Up::Name("  Ada  ".to_owned()));
        game.apply(ada, Up::Tap);
        game.leave(ada);
        assert!(!game.screen("").players[0].here);
        game.join(ada);
        let remote = game.remote(ada).unwrap();
        assert_eq!((remote.name.as_str(), remote.taps), ("Ada", 1));
        assert!(game.screen("").players[0].here);
    }

    #[test]
    fn a_page_that_never_joined_cannot_play() {
        let mut game = Game::default();
        game.apply(Id(7), Up::Tap);
        assert_eq!(game.screen("").players, Vec::new());
    }
}
