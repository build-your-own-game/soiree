// secrets: Oui, mais l'état ne contient aucun Secret (§ 2.12).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
    geste: u8,
    tenu: Aucun,
    vue: u8,
    secrets: Applicable::Oui(()),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: u8,
}

fn main() {}
