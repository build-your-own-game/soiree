//! La machine du jeu : ses règles. Chacun tape, l'écran compte.
//!
//! Rien ici ne connaît le réseau, l'horloge ni le navigateur : la partie ne
//! dépend que des entrées qu'on lui donne ([`Game::join`], [`Game::apply`],
//! [`Game::leave`]). Les mêmes entrées, dans le même ordre, donnent la même
//! partie : [`Game::empreinte`] permet de le vérifier.
//!
//! La machine se passe du système (`#![no_std]`) : l'horloge, les fichiers, le
//! réseau et le hasard du système n'existent pas ici, et `soiree check` s'en
//! assure. Les listes et les textes viennent de `alloc`.

#![no_std]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;

use messages::{Player, Remote, Screen, Up};

/// Un téléphone, pour la machine : son numéro, le même d'une connexion à l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phone(pub u64);

const NAME_MAX: usize = 16;

#[derive(Default)]
pub struct Game {
    seats: BTreeMap<Phone, Seat>,
}

struct Seat {
    name: String,
    taps: u32,
    here: bool,
}

impl Game {
    /// Un téléphone arrive ou revient : il garde sa place et son score.
    pub fn join(&mut self, id: Phone) {
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

    pub fn leave(&mut self, id: Phone) {
        if let Some(seat) = self.seats.get_mut(&id) {
            seat.here = false;
        }
    }

    pub fn apply(&mut self, id: Phone, up: Up) {
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

    pub fn players(&self) -> impl Iterator<Item = Phone> + '_ {
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

    pub fn remote(&self, id: Phone) -> Option<Remote> {
        self.seats.get(&id).map(|seat| Remote {
            name: seat.name.clone(),
            taps: seat.taps,
        })
    }

    /// Un nombre calculé sur tout l'état de la partie : deux parties dans le
    /// même état ont la même empreinte, ici comme dans un navigateur.
    ///
    /// Chaque champ y entre par ses octets, dans un ordre fixé. Pas de
    /// `#[derive(Hash)]` : il compte les longueurs en `usize`, qui n'a pas la
    /// même taille sur l'ordinateur et en WebAssembly.
    pub fn empreinte(&self) -> u64 {
        let mut print = Empreinte::default();
        for (id, seat) in &self.seats {
            print.bytes(&id.0.to_le_bytes());
            print.text(&seat.name);
            print.bytes(&seat.taps.to_le_bytes());
            print.bytes(&[u8::from(seat.here)]);
        }
        print.0
    }
}

/// Une empreinte FNV-1a sur 64 bits : la même sur toutes les machines.
/// Un flottant y entre par ses bits : `print.bytes(&x.to_bits().to_le_bytes())`.
struct Empreinte(u64);

impl Default for Empreinte {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Empreinte {
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Un texte, suivi d'un octet qui ne peut pas en faire partie : « ab » puis
    /// « c » ne se confond pas avec « a » puis « bc ».
    fn text(&mut self, text: &str) {
        self.bytes(text.as_bytes());
        self.bytes(&[0xff]);
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn a_player_keeps_name_and_score_across_a_disconnection() {
        let mut game = Game::default();
        let ada = Phone(1);
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
    fn the_same_inputs_give_the_same_game() {
        let play = || {
            let mut game = Game::default();
            game.join(Phone(2));
            game.join(Phone(1));
            game.apply(Phone(1), Up::Name("Ada".to_owned()));
            game.apply(Phone(2), Up::Tap);
            game.leave(Phone(2));
            game.empreinte()
        };
        assert_eq!(play(), play());
        assert_ne!(play(), Game::default().empreinte());
    }

    #[test]
    fn a_page_that_never_joined_cannot_play() {
        let mut game = Game::default();
        game.apply(Phone(7), Up::Tap);
        assert_eq!(game.screen("").players, Vec::new());
    }
}
