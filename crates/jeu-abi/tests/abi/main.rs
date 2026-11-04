//! Les tests de `jeu-abi` (INTERFACE.md, § 8.1), en un seul binaire.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

mod commun;
mod forme_figee;
mod proprietes;
mod seconde_lecture;
mod strict;
mod valeurs;
mod zones;
