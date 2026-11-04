// telephones: SansObjet, mais un GesteTelephone (§ 2.12).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    geste: u8,
    tenu: Aucun,
    vue: Aucun,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: u8,
}

fn main() {}
