// Un jeu cohérent : écran seul, un secret déclaré.
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    geste: Aucun,
    tenu: Aucun,
    vue: Aucun,
    secrets: Applicable::Oui(()),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: Secret<u8>,
}

fn main() {}
