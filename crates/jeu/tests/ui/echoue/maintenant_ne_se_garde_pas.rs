// `Maintenant` ne se garde pas : ni `Etat`, ni `Clone` (§ 2.4, D15).
use jeu::prelude::*;

#[jeu::etat]
pub struct Garde {
    pub quand: Maintenant<'static>,
}

fn copier(m: &Maintenant<'_>) {
    let _ = m.clone();
}

fn main() {}
