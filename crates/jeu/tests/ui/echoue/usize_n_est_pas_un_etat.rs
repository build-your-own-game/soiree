// `usize` fait 64 bits dans la copie native et 32 dans le module : les mêmes octets
// d'entrée seraient acceptés par l'une et refusés par l'autre. Il n'est ni `Etat` ni
// `Message` ; un jeu écrit `u32` ou `u64`.
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    geste: Aucun,
    tenu: Aucun,
    vue: Aucun,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: usize,
}

fn main() {}
