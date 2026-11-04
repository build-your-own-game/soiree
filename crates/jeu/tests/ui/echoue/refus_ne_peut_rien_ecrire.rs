// Avant d'accepter, le jeu n'a que `&J` : il ne peut ni écrire l'état, ni tirer du
// hasard, ni émettre une sortie ; après un refus, plus rien (§ 2.7, D1 ; PUN-27, HIS-44,
// OUT-171, CAT-94, SQ-25, TL-02).
include!("../commun/minimal.rs");

jeu_minimal! {
    telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
    geste: u8,
    tenu: Aucun,
    vue: u8,
    secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
    champ: u8,
}

fn ecrire_avant(avant: Avant<'_, Minimal>) -> Issue<Minimal> {
    avant.jeu().valeur = 3;
    avant.ignorer()
}

fn tirer_avant(avant: Avant<'_, Minimal>) -> Issue<Minimal> {
    let _ = avant.sorties();
    avant.ignorer()
}

fn ecrire_apres_un_refus(avant: Avant<'_, Minimal>) -> Issue<Minimal> {
    let issue = avant.ignorer();
    let mut apres = avant.accepter();
    apres.jeu().valeur = 3;
    issue
}

fn main() {}
