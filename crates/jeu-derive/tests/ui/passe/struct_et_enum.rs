// Une struct, une enum, un tuple, une enum vide : tout compile (§ 8.2).
use jeu::prelude::*;

#[jeu::message]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[jeu::message]
pub struct Paire(pub u8, pub Point);

#[jeu::message]
pub struct Unite;

#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum Couleur {
    Rouge,
    Vert,
}

#[jeu::message]
pub enum Coup {
    Rien,
    Vers { x: i32, y: i32 },
    Couleur(Couleur),
}

#[jeu::message]
pub enum Vide {}

#[jeu::etat]
pub struct Plateau {
    pub cases: Vec<Option<Couleur>>,
    pub dernier: Option<Coup>,
    pub vitesse: f32,
}

#[jeu::etat]
pub enum Partie {
    Attente,
    EnCours(Plateau),
    Finie { gagnant: Couleur },
}

fn main() {
    let _: &[Couleur] = Couleur::TOUTES;
}
