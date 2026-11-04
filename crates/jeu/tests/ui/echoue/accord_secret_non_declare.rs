// L'état contient un Secret, mais secrets: SansObjet (§ 2.12, réciproque).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
    geste: u8,
    tenu: Aucun,
    vue: u8,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: Secret<u8>,
}

fn main() {}
