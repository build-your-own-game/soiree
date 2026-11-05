//! Les tests de `moteur-commun`, sur une machine fausse en pur Rust (`fausse.rs`) : le
//! protocole s'éprouve sans moteur WebAssembly. Le même protocole, sur wasmtime et de
//! vrais modules, est éprouvé par les tests de `moteur-wasmtime`.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "code de test : des adresses et des compteurs connus, un débordement y serait l'échec du test"
)]

mod fausse;
mod instantane;
mod ordre;
mod releve;

/// Le module de `Bonjour` qui boucle, que le banc lira : un module que la réécriture
/// accepte, sous les réglages de la phase 1.
#[test]
fn le_module_de_bonjour_qui_boucle_se_reecrit() {
    let octets = wat::parse_str(moteur_commun::essais::BONJOUR_QUI_BOUCLE).unwrap();
    let r = reecriture::reecrire(&octets, &reecriture::Reglages::PHASE_1).unwrap();
    assert_eq!(r.pile_initiale, 65_536);
    assert_eq!(r.constat.memoire, (2, Some(2)));
}
