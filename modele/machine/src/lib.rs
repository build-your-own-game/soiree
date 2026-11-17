//! La machine du jeu : ses règles. Chacun tape, l'écran compte.
//!
//! Elle remplit le contrat de la console ([`cartouche::Machine`]) :
//!
//! - tout ce qui lui arrive passe par une seule porte, [`Machine::apply`] : une
//!   page arrive, part, envoie une commande, ou le temps passe ;
//! - tout son état s'écrit et se relit (`Serialize`, `Deserialize`) : la
//!   console en tire l'empreinte de la partie, sa photo et sa reprise ;
//! - elle se passe du système (`#![no_std]`) : l'horloge, les fichiers, le
//!   réseau et le hasard du système n'existent pas ici, et `soiree check` s'en
//!   assure. Les listes et les textes viennent de `alloc`.
//!
//! Les mêmes entrées, dans le même ordre, donnent la même partie. Le banc de
//! la console (`examples/banc.rs`) joue des parties avec elle sans que tu
//! écrives un test ; les tests d'ici disent ce que le jeu doit faire.

#![no_std]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use cartouche::{Input, Machine, Page, Role, To};
use messages::{Effect, Player, Remote, Screen, Up, View};
use serde::{Deserialize, Serialize};

const NAME_MAX: usize = 16;
/// L'écran fête un joueur tous les tant de taps.
const CHEER_EVERY: u32 = 10;

/// La partie. Tout ce qui compte est ici : une page n'a rien à elle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jeu {
    /// Une place par téléphone, sous le numéro de sa page.
    seats: BTreeMap<u64, Seat>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Seat {
    name: String,
    taps: u32,
    here: bool,
}

impl Machine for Jeu {
    type Command = Up;
    type View = View;
    type Effect = Effect;

    /// Ce jeu se passe du temps. Avec un rythme (60, par exemple), la console
    /// envoie `Input::Tick` soixante fois par seconde : la machine compte des
    /// pas, elle ne reçoit jamais l'heure.
    const TICKS_PER_SECOND: u32 = 0;

    /// La graine est tout le hasard du jeu ; celui-ci n'en a pas.
    fn new(_seed: u64) -> Self {
        Self::default()
    }

    fn apply(&mut self, input: Input<Up>) -> Vec<(To, Effect)> {
        match input {
            // Un téléphone arrive ou revient : il garde sa place et son score.
            Input::Joined(page) => {
                if page.role == Role::Remote {
                    let number = self.seats.len().saturating_add(1);
                    self.seats
                        .entry(page.id)
                        .or_insert_with(|| Seat {
                            name: format!("Joueur {number}"),
                            taps: 0,
                            here: true,
                        })
                        .here = true;
                }
                Vec::new()
            }
            Input::Left(page) => {
                if let Some(seat) = self.seats.get_mut(&page.id) {
                    seat.here = false;
                }
                Vec::new()
            }
            Input::Message(page, up) => self.play(page, up),
            Input::Tick => Vec::new(),
        }
    }

    fn view(&self, page: Page) -> Option<View> {
        match page.role {
            Role::Screen => Some(View::Screen(Screen {
                players: self
                    .seats
                    .values()
                    .map(|seat| Player {
                        name: seat.name.clone(),
                        taps: seat.taps,
                        here: seat.here,
                    })
                    .collect(),
            })),
            Role::Remote => self.seats.get(&page.id).map(|seat| {
                View::Remote(Remote {
                    name: seat.name.clone(),
                    taps: seat.taps,
                })
            }),
        }
    }

    /// Ce que les boutons de cette page enverraient maintenant : le banc s'en
    /// sert pour jouer des parties qui avancent.
    fn suggest(&self, page: Page) -> Vec<Up> {
        if self.seats.contains_key(&page.id) {
            vec![Up::Tap, Up::Name("Ada".to_owned())]
        } else {
            Vec::new()
        }
    }
}

impl Jeu {
    /// Une page envoie une commande. Une page peut envoyer n'importe quoi :
    /// ce qui n'a pas de sens est ignoré, sans bruit.
    fn play(&mut self, page: Page, up: Up) -> Vec<(To, Effect)> {
        let Some(seat) = self.seats.get_mut(&page.id) else {
            return Vec::new();
        };
        match up {
            Up::Name(name) => {
                let name: String = name.trim().chars().take(NAME_MAX).collect();
                if !name.is_empty() {
                    seat.name = name;
                }
                Vec::new()
            }
            Up::Tap => {
                seat.taps = seat.taps.saturating_add(1);
                if seat.taps.is_multiple_of(CHEER_EVERY) {
                    // Un effet : l'écran le joue une fois. Le score, lui, est
                    // dans l'état : une page qui rate l'effet ne perd rien.
                    vec![(
                        To::Screens,
                        Effect::Cheer {
                            name: seat.name.clone(),
                            taps: seat.taps,
                        },
                    )]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cartouche::State;

    use super::*;

    const ADA: Page = Page {
        id: 1,
        role: Role::Remote,
    };
    const TV: Page = Page {
        id: 9,
        role: Role::Screen,
    };

    fn players(game: &Jeu) -> Vec<Player> {
        match game.view(TV) {
            Some(View::Screen(screen)) => screen.players,
            Some(View::Remote(_)) | None => Vec::new(),
        }
    }

    #[test]
    fn a_player_keeps_name_and_score_across_a_disconnection() {
        let mut game = Jeu::new(0);
        let _: Vec<_> = game.apply(Input::Joined(ADA));
        let _: Vec<_> = game.apply(Input::Message(ADA, Up::Name("  Ada  ".to_owned())));
        let _: Vec<_> = game.apply(Input::Message(ADA, Up::Tap));
        let _: Vec<_> = game.apply(Input::Left(ADA));
        assert!(!players(&game)[0].here);
        let _: Vec<_> = game.apply(Input::Joined(ADA));
        assert_eq!(
            game.view(ADA),
            Some(View::Remote(Remote {
                name: "Ada".to_owned(),
                taps: 1
            }))
        );
        assert!(players(&game)[0].here);
    }

    #[test]
    fn the_screen_cheers_every_tenth_tap() {
        let mut game = Jeu::new(0);
        let _: Vec<_> = game.apply(Input::Joined(ADA));
        for _ in 0..9 {
            assert_eq!(game.apply(Input::Message(ADA, Up::Tap)), Vec::new());
        }
        assert_eq!(
            game.apply(Input::Message(ADA, Up::Tap)),
            vec![(
                To::Screens,
                Effect::Cheer {
                    name: "Joueur 1".to_owned(),
                    taps: 10
                }
            )]
        );
    }

    #[test]
    fn a_page_that_never_joined_cannot_play() {
        let mut game = Jeu::new(0);
        let _: Vec<_> = game.apply(Input::Message(ADA, Up::Tap));
        let _: Vec<_> = game.apply(Input::Message(TV, Up::Tap));
        assert_eq!(players(&game), Vec::new());
        assert_eq!(game.view(ADA), None);
    }

    #[test]
    fn the_whole_game_is_in_its_state() {
        let mut game = Jeu::new(0);
        let _: Vec<_> = game.apply(Input::Joined(ADA));
        let _: Vec<_> = game.apply(Input::Message(ADA, Up::Tap));
        assert_eq!(Jeu::load(&game.save()), Some(game));
    }
}
