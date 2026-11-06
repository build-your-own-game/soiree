//! Des soirées en mémoire sur le modèle du cœur (PHASE-3, § 8.1, lot 4b) : la vraie
//! autorité, de vrais appareils, le relais en mémoire, à graine fixe (SAL-09).
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

use std::collections::BTreeSet;

use jeu_abi::Oui;
use partie::modele::{FabriqueModele, InstantaneModele};
use salle::simulation::{Pannes, Soiree, verifier_l_historique};

fn soiree(graine: u64, pannes: Pannes) -> Soiree<FabriqueModele, fn() -> FabriqueModele> {
    Soiree::new(
        FabriqueModele::default as fn() -> FabriqueModele,
        graine,
        Oui::Oui,
        pannes,
    )
}

/// Les identités des gestes que le modèle a reçus, dans l'ordre.
fn gestes_du_modele(s: &Soiree<FabriqueModele, fn() -> FabriqueModele>) -> Vec<u64> {
    s.autorite
        .as_ref()
        .and_then(|a| a.partie())
        .and_then(|p| p.definitive().etat())
        .map(|e| e.gestes.clone())
        .unwrap_or_default()
}

#[test]
fn une_soiree_sans_panne_compte_chaque_geste_une_fois() {
    let mut s = soiree(
        1,
        Pannes {
            latence_us: (16_000, 66_000),
            ..Pannes::AUCUNE
        },
    );
    let b = s.jouer(4, 10_000_000);
    eprintln!("{b:#?}");
    assert!(b.arret.is_none(), "{b:?}");
    assert!(b.gestes_emis > 50, "{b:?}");
    assert_eq!(b.gestes_accuses, b.gestes_emis, "{b:?}");
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
    let g = gestes_du_modele(&s);
    let distincts: BTreeSet<u64> = g.iter().copied().collect();
    assert_eq!(distincts.len(), g.len(), "un geste compté deux fois");
    assert_eq!(g.len() as u64, b.gestes_emis);
    let h = verifier_l_historique::<InstantaneModele>(
        &s.cle_de_garde(),
        s.salle(),
        &s.relais.historique(),
    )
    .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    assert_eq!(h.gestes.len() as u64, b.gestes_emis);
}

/// Une soirée avec ses pannes, tirées de la graine : coupures, rechargements, onglets
/// jumeaux, poignées de main rejouées, autorité tuée et relancée depuis la garde.
fn soiree_avec_pannes(graine: u64) {
    let mut s = soiree(graine, Pannes::SOIREE);
    let b = s.jouer(6, 60_000_000);
    eprintln!("graine {graine} : {b:#?}");
    assert!(b.arret.is_none(), "{b:?}");
    assert!(b.redemarrages > 0 && b.coupures > 0, "{b:?}");
    let h = verifier_l_historique::<InstantaneModele>(
        &s.cle_de_garde(),
        s.salle(),
        &s.relais.historique(),
    )
    .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    let g = gestes_du_modele(&s);
    let distincts: BTreeSet<u64> = g.iter().copied().collect();
    assert_eq!(distincts.len(), g.len(), "un geste compté deux fois");
    // Chaque téléphone revenu a tout vu accuser ; un onglet jumeau fermé emporte ce
    // qu'il avait en file, comme un onglet qu'on ferme.
    for (k, t) in s.telephones.iter().enumerate() {
        let jumeau = s.telephones[..k].iter().any(|u| u.jeton == t.jeton);
        if !jumeau {
            assert_eq!(t.appareil.en_file(), 0, "téléphone {k}");
            assert_eq!(
                t.appareil.accuse(),
                t.appareil.dernier_geste(),
                "téléphone {k}"
            );
        }
    }
    // Tout geste accusé est dans le journal écrit, une fois.
    for t in &s.telephones {
        let appareil = s
            .autorite
            .as_ref()
            .and_then(salle::autorite::Autorite::registre)
            .and_then(|r| {
                r.par_jeton(&salle::garde::empreinte_jeton(&t.jeton))
                    .map(|i| i.appareil)
            })
            .expect("l'appareil du téléphone");
        for n in 1..=t.appareil.accuse() {
            let id = salle::autorite::id_geste(appareil, t.appareil.onglet(), n);
            assert!(
                h.gestes.contains(&id.fil()),
                "geste {n} accusé mais pas écrit"
            );
        }
    }
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
}

#[test]
fn une_soiree_avec_ses_pannes_compte_chaque_geste_une_fois_sal57() {
    for graine in 1..=4 {
        soiree_avec_pannes(graine);
    }
}
