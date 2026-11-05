//! Le chargement borné par son carburant, au garde-fou mural de la soirée (PHASE-3, P5 ;
//! RESTE-PHASE-2, point 16).

use moteur_wasmtime::{BUDGET_DE_CHARGEMENT, Moteur, RefusModule, ReglagesMoteur};
use partie::Faute;

/// Le module de `Bonjour` qui boucle (`moteur_commun::essais`), chargé sous
/// `ReglagesMoteur::PHASE_1`, le garde-fou mural d'une seconde que `soiree-epreuve` et
/// la soirée prennent : la faute `Budget` du jeu, jamais une panne de l'outil (HIS-06).
/// [`BUDGET_DE_CHARGEMENT`] est réglé pour que cela tienne même sur une machine
/// chargée : vérifié sous quatorze `yes` qui occupent les dix cœurs de la machine de
/// l'hôte (`MESURES-PHASE-3.md`, « Lot 2 »). Avant le lot 2, à 2³² unités, ce test
/// échouait sans charge : épuiser le budget prenait environ 2 s.
#[test]
fn bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement() {
    let octets = wat::parse_str(moteur_commun::essais::BONJOUR_QUI_BOUCLE).unwrap();
    let r = Moteur::new(ReglagesMoteur::PHASE_1)
        .unwrap()
        .charger(&octets);
    assert_eq!(
        r.err(),
        Some(RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::Budget
        })
    );
    assert_eq!(BUDGET_DE_CHARGEMENT, 1 << 26);
}
