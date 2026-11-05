// Les noms que déclare `Epreuve::INVARIANTS` sont chacun non vides, et chacun une fois :
// E4 juge chaque nom à part (OUT-01). Un nom déclaré deux fois ne compile pas.
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
    geste: u8,
    tenu: Aucun,
    vue: u8,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: u8,
    invariants: &["un point par manche", "un point par manche"],
}

fn main() {}
