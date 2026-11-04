// telephones: Oui, mais aucun type de téléphone (§ 2.12, réciproque).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
    geste: Aucun,
    tenu: Aucun,
    vue: Aucun,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: u8,
}

fn main() {}
