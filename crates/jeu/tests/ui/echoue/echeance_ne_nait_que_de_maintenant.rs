// Une `Echeance` ne naît que depuis `Maintenant` : pas de constructeur depuis un pas
// (§ 2.4, PUN-34).
use jeu::prelude::*;

fn main() {
    let _ = Echeance(Pas::ZERO);
}
