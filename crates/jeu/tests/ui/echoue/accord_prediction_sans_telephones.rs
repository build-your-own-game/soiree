// prediction: Oui demande telephones: Oui (§ 2.12).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    geste: Aucun,
    tenu: Aucun,
    vue: Aucun,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::Oui(()),
    champ: u8,
}

fn main() {}
