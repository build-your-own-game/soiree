//! Les tests de `reecriture` (INTERFACE.md, § 8.5), en un seul binaire.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]
#![expect(
    clippy::wildcard_enum_match_arm,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "des valeurs de test tirées d'une graine ; un variant inattendu fait échouer le test"
)]

mod commun;
mod passes;
mod refus;
mod sens;
