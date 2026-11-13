//! Joue des parties écrites d'avance et affiche l'empreinte de leur état.
//!
//! `soiree check` lance ce programme sur l'ordinateur, deux fois, puis en
//! WebAssembly, et compare les lignes : la machine doit jouer la même partie
//! partout. Quand le jeu grandit, fais grandir ces parties : tous les coups
//! du jeu, plusieurs graines s'il y a du hasard, des entrées absurdes aussi.

use machine::{Game, Phone};
use messages::Up;

fn show(game: &Game, party: &str, step: &str) {
    println!("{party}, {step} : {:016x}", game.empreinte());
}

fn main() {
    let party = "trois joueurs";
    let mut game = Game::default();
    show(&game, party, "vide");
    for phone in [3, 1, 2] {
        game.join(Phone(phone));
    }
    show(&game, party, "arrivés");
    game.apply(Phone(1), Up::Name("  Ada  ".to_owned()));
    game.apply(
        Phone(2),
        Up::Name("Zoé, qui a un prénom beaucoup trop long".to_owned()),
    );
    game.apply(Phone(3), Up::Name(String::new()));
    show(&game, party, "nommés");
    for phone in [1, 2, 3].into_iter().cycle().take(1_000) {
        game.apply(Phone(phone), Up::Tap);
    }
    show(&game, party, "mille taps");
    game.leave(Phone(2));
    game.apply(Phone(2), Up::Tap);
    show(&game, party, "un départ");
    game.join(Phone(2));
    show(&game, party, "un retour");

    let party = "des inconnus";
    let mut game = Game::default();
    game.apply(Phone(7), Up::Tap);
    game.leave(Phone(8));
    game.apply(Phone(u64::MAX), Up::Name("personne".to_owned()));
    show(&game, party, "rien ne change");
}
