//! A tiny deterministic game: each player taps and the TV counts.
//! Time, identities and actions enter explicitly; nothing reads a clock.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod protocol;
pub mod replay;

/// The room capacity is a game rule, shared with the host.
pub const MAX_PLAYERS: usize = 8;

pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    players: BTreeMap<u32, Seat>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub name: String,
    pub taps: u32,
    pub here: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    Join { player: u32, name: String },
    Presence { player: u32, here: bool },
    Tap { player: u32 },
}

/// A counter never wraps, even in the longest game.
pub fn next_score(score: u32) -> Option<u32> {
    score.checked_add(1)
}

impl Game {
    pub fn players(&self) -> &BTreeMap<u32, Seat> {
        &self.players
    }

    pub fn apply(&mut self, input: Input) {
        match input {
            Input::Join { player, name } => {
                if self.players.len() < MAX_PLAYERS {
                    self.players.entry(player).or_insert(Seat {
                        name,
                        taps: 0,
                        here: true,
                    });
                }
            }
            Input::Presence { player, here } => {
                if let Some(seat) = self.players.get_mut(&player) {
                    seat.here = here;
                }
            }
            Input::Tap { player } => {
                if let Some(seat) = self.players.get_mut(&player).filter(|seat| seat.here) {
                    seat.taps = next_score(seat.taps).unwrap_or(u32::MAX);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn score_increments_without_wrapping() {
        assert_eq!(next_score(0), Some(1));
        assert_eq!(next_score(42), Some(43));
        assert_eq!(next_score(u32::MAX), None);
    }

    #[test]
    fn absent_players_cannot_score_and_return_keeps_score() {
        let mut game = Game::default();
        game.apply(Input::Join {
            player: 1,
            name: "Ada".into(),
        });
        game.apply(Input::Tap { player: 1 });
        game.apply(Input::Presence {
            player: 1,
            here: false,
        });
        game.apply(Input::Tap { player: 1 });
        game.apply(Input::Presence {
            player: 1,
            here: true,
        });
        assert_eq!(game.players()[&1].taps, 1);
    }

    #[test]
    fn capacity_is_enforced_in_the_simulation() {
        let mut game = Game::default();
        for player in 0..20 {
            game.apply(Input::Join {
                player,
                name: format!("Bot {player}"),
            });
        }
        assert_eq!(game.players().len(), MAX_PLAYERS);
        assert!(!game.players().contains_key(&19));
    }

    proptest! {
        #[test]
        fn inputs_replay_identically(presses in prop::collection::vec(0_u32..8, 0..200)) {
            let mut game = Game::default();
            let mut entries = Vec::new();
            for player in 0..8 {
                let input = Input::Join { player, name: format!("Bot {player}") };
                game.apply(input.clone());
                entries.push(replay::Entry { at: 0, input });
            }
            for &player in &presses {
                let input = Input::Tap { player };
                game.apply(input.clone());
                entries.push(replay::Entry { at: 1, input });
            }
            for player in 0..8 {
                let expected = u32::try_from(presses.iter().filter(|&&pressed| pressed == player).count()).unwrap();
                prop_assert_eq!(game.players()[&player].taps, expected);
            }
            let hash = replay::fingerprint(&game).unwrap();
            let replayed = replay::replay(&replay::Header::default(), &entries, &hash).unwrap();
            prop_assert_eq!(game, replayed);
        }
    }
}
