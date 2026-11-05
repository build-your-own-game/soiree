// Un regard ne se prête que le temps d'un appel (§ 2.8) : ni `Clone`, ni `Copy`. Une
// vue ne peut pas garder le regard d'un joueur pour ouvrir ses secrets sous le regard
// de la télé, à l'appel suivant.
use jeu::prelude::*;

fn copier(r: &Regard) -> Regard {
    *r
}

fn cloner(r: &Regard) -> Regard {
    r.clone()
}

fn main() {}
