//! Les tests du cœur de la partie qui fait foi (INTERFACE.md, § 8.6), sur le
//! modèle `partie::modele`. Un seul binaire, pour que les aides de `commun` servent
//! à tous.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »). L'arithmétique des scénarios porte sur de petits nombres de pas :
//! un débordement y serait un bug du test, qui panique et échoue.

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "les scénarios comptent des pas et des numéros petits ; un débordement fait échouer le test"
)]

mod commun;
mod fenetre;
mod partie;
mod proprietes;
mod regles;
