// `Secret`, `Partage` et `Appareils` vivent dans un état (§ 8.2).
use jeu::prelude::*;

#[jeu::etat]
pub struct Main {
    pub cartes: Secret<Vec<u8>>,
    pub image: Partage<Vec<u8>>,
    pub joueurs: Appareils<Secret<u32>>,
}

fn main() {
    assert!(<Main as jeu::Forme>::SECRET == jeu::Contient::Oui);
}
